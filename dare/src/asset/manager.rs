use super::prelude as asset;
use crate::asset::asset::{AssetDescriptor, AssetHolder, AssetState};
use crate::asset::prelude::AssetUnloaded;
use crate::render;
use crate::render::transfer::{BufferTransferRequest, TransferRequest};
use anyhow::Result;
use containers::dashmap::DashMap;
use dagal::allocators::{Allocator, ArcAllocator, MemoryLocation};
use dagal::ash::vk;
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use dare_containers::prelude as containers;
use futures::StreamExt;
use rayon::prelude::*;
use std::any::TypeId;
use std::marker::PhantomData;
use std::sync::Arc;

pub struct AssetContainerSlot<T: 'static + AssetDescriptor> {
    ttl: usize,
    t: usize,
    holder: AssetHolder<T>,
}

/// Quick access to the underlying asset container type
pub type AssetContainer<T: AssetDescriptor> = DashMap<T::Metadata, AssetContainerSlot<T>>;

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
    /// GPU Resource Table
    gpu_rt: GPUResourceTable<A>,
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
pub struct BufferRequest<A: Allocator + 'static> {
    buffer_usage: vk::BufferUsageFlags,
    metadata: super::buffer::BufferMetaData<A>,
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
        gpu_rt: GPUResourceTable<A>,
        ttl: usize,
    ) -> Result<Self> {
        Ok(Self {
            cache: Arc::new(containers::ErasedStorageDashMap::new()),
            allocator,
            transfer,
            gpu_rt,
            inner: Arc::new(AssetManagerInner {
                ttl,
            }),
            _marker: PhantomData,

        })
    }

    /// Loads an asset in
    pub fn insert<T: 'static + AssetDescriptor + PartialEq>(
        &mut self,
        metadata: T::Metadata,
    ) -> Result<()> {
        if !self.cache.contains_key::<AssetContainer<T>>() {
            self.cache
                .insert::<AssetContainer<T>>(AssetContainer::new());
        }

        self.cache
            .with_mut::<AssetContainer<T>, _, _>(|map| {
                map.insert(metadata.clone(), AssetContainerSlot {
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
        &self,
        metadata: &T::Metadata,
    ) -> Option<AssetHolder<T>> {
        if !self.cache.contains_key::<AssetContainer<T>>() {
            None
        } else {
            self.cache
                .with_mut::<AssetContainer<T>, _, _>(|map| map.remove(metadata))
                .flatten()
                .map(|tuple| tuple.1.holder)
        }
    }

    /// Get
    pub fn get<T: 'static + AssetDescriptor>(&self, metadata: &T::Metadata) -> Option<AssetHolder<T>> {
        if !self.cache.contains_key::<AssetContainer<T>>() {
            None
        } else {
            self.cache
                .with_mut::<AssetContainer<T>, _, _>(|map| map.get(metadata).map(|resource| resource.holder.clone()))
                .flatten()
        }
    }

    /// Update life times
    pub fn update(&self) -> Result<()> {
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

    pub async fn remove_expired_slots(&self) -> Result<()> {
        let _ = self.cache.iter().map(|map| async move {
            if let Some(map) = map.downcast_ref::<AssetContainer<asset::Buffer<A>>>() {
                for container in map.iter_mut() {
                    if container.t == 0 {
                        let mut write_guard = container.holder.state.write().await;
                        let asset = match &*write_guard {
                            AssetState::Loaded(asset) => Arc::downgrade(asset),
                            _ => unimplemented!()
                        };
                        *write_guard = AssetState::Unloading(asset);
                    }
                }
            } else if let Some(map) = map.downcast_ref::<AssetContainer<asset::Image<A>>>() {
                for container in map.iter_mut() {
                    if container.t == 0 {
                        let mut write_guard = container.holder.state.write().await;
                        let asset = match &*write_guard {
                            AssetState::Loaded(asset) => Arc::downgrade(asset),
                            _ => unimplemented!()
                        };
                        *write_guard = AssetState::Unloading(asset);
                    }
                }
            }
        });
        Ok(())
    }

    /// Attempts to get a slot loaded
    ///
    /// `autoload` determines if a slot should be loaded
    pub async fn get_slot_loaded<T: AssetDescriptor + 'static>(&self, metadata: &T::Metadata, load_info: Option<<T::Metadata as AssetUnloaded>::LoadInfo>) -> Result<Arc<T::Loaded>> {
        let container = self.cache.get::<AssetContainer<T>>().map_or(Err(anyhow::Error::new(asset::error::AssetMetadataNone)), |a| Ok(a))?;
        let state = container.value().downcast_ref::<AssetContainer<T>>().unwrap()
                             .get(metadata)
                             .map_or(Err(anyhow::Error::new(asset::error::AssetMetadataNone)), |slot| {
                                 Ok(slot.holder.state.clone())
                             })?;
        let state_guard = state.read().await;
        let resource: Option<Arc<<T::Metadata as AssetUnloaded>::AssetLoaded>> = match &*state_guard {
            AssetState::Unloaded(metadata) => match &load_info {
                Some(_) => None,
                None => return Err::<Arc<T::Loaded>, anyhow::Error>(anyhow::Error::new(asset::error::AssetNotLoaded)),
            }
            AssetState::Loading(loading) => {
                let mut loading = loading.clone();
                loading.changed().await?;
                let image_option = loading.borrow_and_update();
                return match image_option.as_ref() {
                    None => Err::<Arc<T::Loaded>, anyhow::Error>(anyhow::Error::new(asset::error::AssetNotLoaded)),
                    Some(loaded) => return Ok::<Arc<T::Loaded>, anyhow::Error>(loaded.clone()),
                }
            }
            AssetState::Loaded(loaded) => return Ok(loaded.clone()),
            AssetState::Unloading(unloading) => match unloading.upgrade() {
                None => None,
                Some(loaded) => {
                    Some(loaded.clone())
                },
            }
        };
        if resource.is_none() && load_info.is_none() {
            return Err(anyhow::Error::new(asset::error::AssetNotLoaded));
        }
        drop(state_guard);
        let resource = match resource {
            Some(resource) => resource,
            None => {
                let mut state_guard = state.write().await;
                let (send, recv) = tokio::sync::watch::channel(None);
                *state_guard = AssetState::Loading(recv);
                drop(state_guard);
                let resource = metadata.load(load_info.unwrap(), send).await?;
                resource
            }
        };
        // update type
        let mut state_guard = state.write().await;
        *state_guard = AssetState::Loaded(resource.clone());
        drop(state_guard);
        Ok(resource)
    }
}

impl<A: Allocator + 'static> AssetManager<A> {
    /// Retrieve a buffer and load it automatically or wait until the buffer is available
    /// Only target format is used if we're loading an entirely new file
    pub async fn retrieve_buffer(
        &self,
        load_request: BufferRequest<A>,
    ) -> Result<Arc<resource::Buffer<A>>> {
        let notify = Arc::new(tokio::sync::Notify::new());
        let container_ref = self.cache.handle().entry(TypeId::of::<AssetContainer<asset::Buffer<A>>>())
                                .or_insert(Box::<AssetContainer<asset::Buffer<A>>>::new(AssetContainer::new()));
        let container = container_ref.value()
                                     .downcast_ref::<AssetContainer<asset::Buffer<A>>>()
                                     .unwrap();
        let mut allocator = self.allocator.clone();

        let res: Result<Arc<resource::Buffer<A>>> = match container.get_mut(&load_request.metadata) {
            None => unimplemented!(),
            Some(slot) => {
                let metadata = slot.holder.metadata.clone();
                let slot = slot.holder.state.clone();
                let state = slot.read().await;
                if let AssetState::Unloaded(_) = &*state {
                    drop(state);
                    let (sender, reciever) = tokio::sync::watch::channel::<Option<Arc<resource::Buffer<A>>>>(None);
                    {
                        let mut slot_write_guard = slot.write().await;
                        *slot_write_guard = AssetState::Loading(reciever.clone())
                    }

                    let mut chunk_buffer = resource::Buffer::new(
                        resource::BufferCreateInfo::NewEmptyBuffer {
                            device: self.allocator.device(),
                            allocator: &mut allocator,
                            size: load_request.chunk_size as vk::DeviceSize,
                            memory_type: MemoryLocation::CpuToGpu,
                            usage_flags: vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::TRANSFER_SRC,
                        },
                    )?;
                    let buffer = resource::Buffer::new(
                        resource::BufferCreateInfo::NewEmptyBuffer {
                            device: self.allocator.device(),
                            allocator: &mut allocator,
                            size: (metadata.element_format.size() * metadata.element_count) as vk::DeviceSize,
                            memory_type: MemoryLocation::GpuOnly,
                            usage_flags: load_request.buffer_usage | vk::BufferUsageFlags::TRANSFER_DST,
                        },
                    )?;

                    let mut dst_offset: vk::DeviceSize = 0;
                    while let Some(chunk) = metadata
                        .clone()
                        .stream(super::buffer::BufferStreamInfo {
                            chunk_size: load_request.chunk_size,
                        })
                        .await?
                        .next()
                        .await
                    {
                        let chunk = chunk?;
                        let chunk_size = chunk.len();
                        chunk_buffer.write(0, &chunk)?;
                        unsafe {
                            self.transfer
                                .transfer_gpu(TransferRequest::Buffer(BufferTransferRequest {
                                    src_buffer: *chunk_buffer.as_raw(),
                                    dst_buffer: *buffer.as_raw(),
                                    src_offset: 0,
                                    dst_offset,
                                    length: chunk_size as vk::DeviceSize,
                                }))
                                .await?;
                        }
                        dst_offset += chunk_size as vk::DeviceSize;
                    }

                    let buffer = Arc::new(buffer);
                    {
                        let mut slot_write_guard = slot.write().await;
                        if let AssetState::Loading(_) = &*slot_write_guard {
                            sender.send(Some(buffer.clone()))?;
                        }
                        *slot_write_guard = AssetState::Loaded(buffer.clone());
                    }
                    Ok(buffer)
                } else if let AssetState::Loading(notify) = &*state {
                    let mut notify = notify.clone();
                    drop(state);
                    notify.changed().await?;
                    let x = Ok(notify.borrow().clone().unwrap().clone());
                    x
                } else if let AssetState::Loaded(buffer) = &*state {
                    Ok(buffer.clone())
                } else if let AssetState::Unloading(buffer) = &*state {
                    let buffer = buffer.clone();
                    drop(state);
                    let mut slot_write_guard = slot.write().await;
                    if let Some(buffer) = buffer.upgrade() {
                        *slot_write_guard = AssetState::Loaded(buffer.clone());
                        Ok(buffer.clone())
                    } else {
                        *slot_write_guard = AssetState::Unloaded(metadata.clone());
                        Ok(self.retrieve_buffer(load_request).await?)
                    }
                } else {
                    unimplemented!()
                }
            }
        };
        drop(container_ref);
        res
    }
}
