use super::prelude as asset;
use crate::asset::asset::{AssetDescriptor, AssetHolder, AssetState, AssetUnloaded};
use crate::asset::buffer::BufferMetaData;
use crate::render;
use crate::render::transfer::{BufferTransferRequest, TransferRequest};
use anyhow::Result;
use containers::dashmap::DashMap;
use dagal::allocators::{Allocator, ArcAllocator};
use dagal::ash::vk;
use dagal::resource;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use dare_containers::prelude as containers;
use futures::StreamExt;
use rayon::prelude::*;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

pub struct AssetContainerSlot<T: 'static + AssetDescriptor> {
    ttl: usize,
    t: usize,
    holder: AssetHolder<T>,
}

/// Quick access to the underlying asset container type
pub type AssetContainer<T: 'static> = DashMap<String, AssetContainerSlot<T>>;

#[derive(Debug)]
pub struct AssetManagerInner {
    ttl: usize,
}

/// Manages loading assets in and out of the gpu
#[derive(Clone, Debug)]
pub struct AssetManager<A: Allocator> {
    cache: Arc<containers::ErasedStorageDashMap>,
    /// Allocator
    allocator: ArcAllocator<A>,
    /// Transfer
    transfer: render::transfer::TransferPool,
    /// Inner
    inner: Arc<AssetManagerInner>,
    _marker: PhantomData<A>,
}

#[derive(Debug)]
pub enum AssetError {
    AssetLoading(Arc<tokio::sync::Notify>),
    Other(anyhow::Error),
}

#[derive(Debug)]
pub struct BufferRequest {
    buffer_usage: vk::BufferUsageFlags,
    target_format: String,
    chunk_size: usize,
}

impl From<anyhow::Error> for AssetError {
    fn from(value: anyhow::Error) -> Self {
        AssetError::Other(value)
    }
}

impl<A: Allocator + 'static> AssetManager<A> {
    pub fn new(
        allocator: ArcAllocator<A>,
        transfer: render::transfer::TransferPool,
        ttl: usize,
    ) -> Result<Self> {
        Ok(Self {
            cache: Arc::new(containers::ErasedStorageDashMap::new()),
            allocator,
            transfer,
            inner: Arc::new(AssetManagerInner {
                ttl,
            }),
            _marker: PhantomData,

        })
    }

    /// Loads an asset in
    pub fn insert<T: 'static + AssetDescriptor + PartialEq>(
        &mut self,
        name: String,
        metadata: T::Metadata,
    ) -> Result<()> {
        if !self.cache.contains_key::<AssetContainer<T>>() {
            self.cache
                .insert::<AssetContainer<T>>(AssetContainer::new());
        }

        self.cache
            .with_mut::<AssetContainer<T>, _, _>(|map| {
                map.insert(name, AssetContainerSlot {
                    ttl: self.inner.ttl,
                    t: self.inner.ttl,
                    holder: AssetHolder::new(metadata)
                });
                Ok(())
            })
            .unwrap()
    }

    /// Removes an asset entirely from cache, but may still exist on GPU memory until it becomes unused
    /// by the GPU
    pub fn remove<T: 'static + AssetDescriptor>(
        &mut self,
        asset_name: String,
    ) -> Option<AssetHolder<T>> {
        if !self.cache.contains_key::<AssetContainer<T>>() {
            None
        } else {
            self.cache
                .with_mut::<AssetContainer<T>, _, _>(|map| map.remove(&asset_name))
                .flatten()
                .map(|tuple| tuple.1.holder)
        }
    }

    /// Update life times
    pub fn update(&mut self) -> Result<()> {
        let _ = self.cache.iter().map(|map| {
            if let Some(map) = map.downcast_ref::<AssetContainer<asset::Buffer<A>>>() {
                for mut container in map.iter_mut() {
                    container.t -= 1;
                }
            } else if let Some(map) = map.downcast_ref::<AssetContainer<asset::Image<A>>>() {
                for mut container in map.iter_mut() {
                    container.t -= 1;
                }
            }
        });
        Ok(())
    }
}

impl<A: Allocator + 'static> AssetManager<A> {
    /// Retrieve a buffer and load it automatically or wait until the buffer is available
    /// Only target format is used if we're loading an entirely new file
    pub async fn retrieve_buffers(
        &mut self,
        load_requests: Vec<BufferRequest>,
    ) -> Result<Arc<RwLock<resource::Buffer<A>>>> {
        let _ = load_requests.into_iter().filter_map(|request| {
            let notify = Arc::new(tokio::sync::Notify::new());
            let metadata = self.cache.with_mut::<AssetContainer<asset::Buffer<A>>, _, _>(|entry| {
                entry.get_mut(&request.target_format).map(|data| {
                    let data = &data.holder;
                    let mut state_guard = data.state.write().map_err(|_| dagal::DagalError::PoisonError)?;
                    match *state_guard {
                        AssetState::Unloaded => {
                            *state_guard = AssetState::Loading(notify);
                            Ok(data.metadata.clone())
                        }
                        _ => {
                            Err(anyhow::anyhow!("Failed to load asset request due to it already being loaded"))
                        },
                    }
                })
            })??.unwrap();
            {
                resource::Buffer::new(
                    resource::BufferCreateInfo::NewEmptyBuffer {
                        device: self.allocator.device(),
                        allocator: &mut self.allocator,
                        size: (metadata.element_format.size() * metadata.element_count) as vk::DeviceSize,
                        memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                        usage_flags: request.buffer_usage,
                    }
                ).map(|buffer| (buffer, request, metadata)).ok()
            }
        }).collect::<Vec<(resource::Buffer<A>, BufferRequest, BufferMetaData<A>)>>()
                             .into_par_iter()
                             .map(|(mut buffer, request, metadata)| {
                                 let transfer_pool = self.transfer.clone();
                                 let transfer_buffer = resource::Buffer::new(
                                     resource::BufferCreateInfo::NewEmptyBuffer {
                                         device: self.allocator.device(),
                                         allocator: &mut self.allocator.clone(),
                                         size: request.chunk_size as vk::DeviceSize,
                                         memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                                         usage_flags: request.buffer_usage,
                                     }).unwrap();
                                 async move {
                                     let mut stream = metadata.clone().stream(super::buffer::BufferLoadInfo {
                                         chunk_size: request.chunk_size
                                     }).await?;
                                     let mut dst_offset: vk::DeviceSize = 0;
                                     while let Some(chunk) = stream.next().await {
                                         let chunk = chunk?;
                                         buffer.write(0, &chunk)?;
                                         unsafe {
                                             // TODO: the transfer pool is still highly limiting due to how it is not capable of batching submissions
                                             transfer_pool.transfer_gpu(TransferRequest::Buffer(BufferTransferRequest {
                                                 src_buffer: *transfer_buffer.as_raw(),
                                                 dst_buffer: *buffer.as_raw(),
                                                 src_offset: 0,
                                                 dst_offset,
                                                 length: buffer.get_size(),
                                             })).await?;
                                         }
                                         dst_offset += chunk.len() as vk::DeviceSize;
                                     }
                                     // update the buffer state
                                     Ok::<(), anyhow::Error>(())
                                 }
                             });
        /*
        let metadata = self.cache.with::<DashMap<String, AssetHolder<asset::buffer::Buffer<A>>>, _, _>(|dashmap| {
            Some(dashmap.get(&asset_name)?.metadata.clone())
        }).flatten();
        let stream = self.stream::<asset::buffer::Buffer<A>>(asset_name.clone(), asset_stream_info).await;
        match stream {
            Ok(stream) => {
                if let Some(metadata) = metadata {
                    match asset::buffer::BufferMetaData::<A>::cast_stream(Ok(stream), metadata.element_format, target_format).await {
                        Ok(mut stream) => {
                            // Make a new buffer
                            let buffer = resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                                device: self.allocator.device(),
                                allocator: &mut self.allocator,
                                size: (metadata.element_count * metadata.element_format.size()) as vk::DeviceSize,
                                memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                                usage_flags: Default::default(),
                            })?;
                            while let Some(chunk) = stream.next().await {
                                let chunk = chunk?;
                            }
                            Ok(todo!())
                        }
                        Err(err) => {
                            Err(err)
                        }
                    }
                } else {
                    Err(anyhow::anyhow!("Did not find {} in asset storage", asset_name))
                }
            }
            Err(err) => match err {
                AssetError::AssetLoading(loading) => unimplemented!(),
                AssetError::Other(err) => {
                    Err(err)
                }
            }
        }
         */
        todo!()
    }
}
