pub mod slot;

use super::prelude as asset;
use crate::asset::prelude::AssetUnloaded;
use crate::render;
use crate::render::transfer::{BufferTransferRequest, TransferRequest};
use crate::util::either::Either;
use anyhow::Result;
use containers::dashmap::DashMap;
use dagal::allocators::{Allocator, ArcAllocator, MemoryLocation};
use dagal::ash::vk;
use dagal::descriptor::GPUResourceTable;
use dagal::resource;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use dare_containers::prelude as containers;
use futures::{FutureExt, SinkExt, StreamExt};
use rayon::prelude::*;
use std::any::{Any, TypeId};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use tokio::io::AsyncReadExt;

/// Contains the metadata hash of an [`AssetDescriptor`]
///
/// It is unique as the hash implementation will directly write the u64 stored in it
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub struct MetadataHash(u64);
impl Hash for MetadataHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0);
    }
}
impl Into<u64> for MetadataHash {
    fn into(self) -> u64 {
        self.0
    }
}
impl From<u64> for MetadataHash {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Quick access to the underlying asset container type
pub type AssetContainer<T: asset::AssetDescriptor> = DashMap<u64, slot::AssetContainerSlot<T>>;

#[derive(Debug)]
pub struct AssetManagerInner {
    ttl: usize,
    keys: Box<[TypeId]>,
}

/// Manages loading assets in and out of the gpu. Uses erased storage to abstract over
///
/// # Static
/// A [`AssetManager`] has static keys into the initial erased storage type, therefore, adding
/// new asset types is not supported nor possible
#[derive(Clone, Debug, bevy_ecs::prelude::Resource)]
pub struct AssetManager<A: Allocator> {
    cache: containers::erased_storage::FlashMapErasedStorage,
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
    pub buffer_usage: vk::BufferUsageFlags,
    pub metadata: super::buffer::BufferMetaData<A>,
    pub chunk_size: usize,
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
        keys: Vec<TypeId>,
        ttl: usize,
    ) -> Result<Self> {
        let cache = containers::erased_storage::FlashMapErasedStorage::new();
        Ok(Self {
            cache,
            allocator,
            transfer,
            gpu_rt,
            inner: Arc::new(AssetManagerInner {
                ttl,
                keys: keys.into_boxed_slice(),
            }),
            _marker: PhantomData,
        })
    }

    /// Transitions asset from [`asset::AssetState::Loaded`] to [`asset::AssetState::Unloading`]
    ///
    /// Also transitions assets from [`asset::AssetState::Unloading`] to [`asset::AssetState::Unloaded`]
    pub async fn update_dead_assets<T: asset::AssetDescriptor + 'static>(
        containers: containers::erased_storage::FlashMapErasedStorage,
    ) -> Result<()> {
        containers.with::<AssetContainer<T>, _, _>(|container| {
            for pair in container.iter() {
                let slot = pair.value();
                if slot.t.load(Ordering::Acquire) == 0 {
                    // Try to obtain a read lock, if one cannot be obtained, just give up
                    if let Ok(mut state) = slot.holder.state.try_write() {
                        let mut state = &mut *state;
                        match state {
                            asset::AssetState::Unloaded(_) => {}
                            asset::AssetState::Loading(_) => {}
                            asset::AssetState::Loaded(arc) => {
                                let weak = Arc::downgrade(&arc);
                                *state = asset::AssetState::Unloading(weak);
                            }
                            asset::AssetState::Unloading(weak) => {
                                if Weak::upgrade(weak).is_none() {
                                    *state = asset::AssetState::Unloaded(
                                        slot.holder.metadata.clone(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        });
        Ok(())
    }

    /// Loads an asset in
    pub fn insert<T: 'static + asset::AssetDescriptor + PartialEq>(
        &mut self,
        metadata: T::Metadata,
    ) -> Result<slot::AssetContainerSlot<T>> {
        self.cache
            .with::<AssetContainer<T>, _, _>(|map| {
                let hash = {
                    let mut hasher = std::hash::DefaultHasher::new();
                    metadata.hash(&mut hasher);
                    hasher.finish()
                };
                let asset_holder = asset::AssetMetadataAndState::new(metadata.clone());
                let container_slot: slot::AssetContainerSlot<T> = slot::AssetContainerSlot {
                    ttl: self.inner.ttl,
                    t: Arc::new(AtomicUsize::new(self.inner.ttl)),
                    holder: asset_holder.clone(),
                };
                map.insert(hash, container_slot.clone());
                container_slot
            })
            .map_or(
                Err(anyhow::Error::from(anyhow::anyhow!("Key does not exist"))),
                Ok,
            )
    }

    /// Get a clone of the underlying [`slot::AssetContainerSlot`]
    pub fn get<'a, T: 'static + asset::AssetDescriptor>(
        &self,
        metadata: Either<&MetadataHash, &T::Metadata>,
    ) -> Option<slot::AssetContainerSlot<T>> {
        self.cache
            .with::<AssetContainer<T>, _, _>(move |map| {
                let mut hash = std::hash::DefaultHasher::new();
                metadata.hash(&mut hash);
                let hash = hash.finish();
                map.get(&hash).map(move |resource| resource.clone())
            })
            .flatten()
    }

    /// Get a [`slot::AssetSlotRef`] to a resource
    ///
    /// This is more performant and if you do not need metadata, this is better to choose
    pub fn get_ref<T: 'static + asset::AssetDescriptor>(
        &self,
        key: Either<&MetadataHash, &T::Metadata>,
    ) -> Option<slot::AssetSlotRef<T>> {
        self.cache
            .with::<AssetContainer<T>, _, _>(move |map| {
                let mut hash = std::hash::DefaultHasher::new();
                key.hash(&mut hash);
                let hash = hash.finish();
                map.get(&hash)
                    .map(move |resource| slot::AssetSlotRef::from(resource.value()))
            })
            .flatten()
    }

    /// Gets metadata
    pub fn get_metadata_from_ref<T: 'static + asset::AssetDescriptor>(
        &self,
        slot: Either<&MetadataHash, &T::Metadata>,
    ) -> Option<T::Metadata> {
        self.cache
            .with::<AssetContainer<T>, _, _>(|map| {
                let hash = {
                    let mut hasher = std::hash::DefaultHasher::new();
                    slot.hash(&mut hasher);
                    hasher.finish()
                };
                map.get(&hash)
                    .map(|resource| resource.get_holder().metadata.clone())
            })
            .flatten()
    }

    /// Attempts to get a slot loaded
    ///
    /// `autoload` determines if a slot should be loaded
    pub async fn get_slot_loaded_with_hash<T: asset::AssetDescriptor + 'static>(
        &self,
        hash: Either<&MetadataHash, &T::Metadata>,
        load_info: Option<<T::Metadata as AssetUnloaded>::LoadInfo>,
    ) -> Result<Arc<T::Loaded>> {
        let asset_ref = self
            .get::<T>(hash)
            .map_or(Err(anyhow::Error::from(asset::error::AssetNotLoaded)), Ok)?;

        #[derive(Debug)]
        pub enum ResourceLoaded<T: asset::AssetDescriptor + 'static> {
            /// Found a pre-existing loaded asset
            Loaded(Arc<<T::Metadata as AssetUnloaded>::AssetLoaded>),
            /// Were able to convert an unloading state to loaded
            UnloadedLoaded(Arc<<T::Metadata as AssetUnloaded>::AssetLoaded>),
            /// No asset found, need to load in
            None,
        }
        let state_guard = asset_ref.get_holder().state.read().await;
        let resource: ResourceLoaded<T> = match &*state_guard {
            asset::AssetState::Unloaded(metadata) => match &load_info {
                Some(_) => ResourceLoaded::None,
                None => {
                    return Err::<Arc<T::Loaded>, anyhow::Error>(anyhow::Error::new(
                        asset::error::AssetNotLoaded,
                    ))
                }
            },
            asset::AssetState::Loading(loading) => {
                let mut loading = loading.clone();
                loading.changed().await?;
                let image_option = loading.borrow_and_update();
                return match image_option.as_ref() {
                    None => Err::<Arc<T::Loaded>, anyhow::Error>(anyhow::Error::new(
                        asset::error::AssetNotLoaded,
                    )),
                    Some(loaded) => return Ok::<Arc<T::Loaded>, anyhow::Error>(loaded.clone()),
                };
            }
            asset::AssetState::Loaded(loaded) => return Ok(loaded.clone()),
            asset::AssetState::Unloading(unloading) => match unloading.upgrade() {
                None => ResourceLoaded::None,
                Some(loaded) => ResourceLoaded::UnloadedLoaded(loaded.clone()),
            },
        };
        if load_info.is_none() {
            return Err(anyhow::Error::new(asset::error::AssetNotLoaded));
        }
        drop(state_guard);
        let resource = match resource {
            ResourceLoaded::Loaded(resource) => resource,
            ResourceLoaded::UnloadedLoaded(resource) => {
                // if we found an unloading state and loaded it back, set it back to loaded
                let mut state_guard = asset_ref.get_holder().state.write().await;
                *state_guard = asset::AssetState::Loaded(resource.clone());
                drop(state_guard);
                resource
            }
            ResourceLoaded::None => {
                let mut state_guard = asset_ref.get_holder().state.write().await;
                let (send, recv) = tokio::sync::watch::channel(None);
                *state_guard = asset::AssetState::Loading(recv);
                drop(state_guard);
                let resource = asset_ref
                    .get_holder()
                    .metadata
                    .load(load_info.unwrap(), send)
                    .await?;
                resource
            }
        };
        // update type
        let mut state_guard = asset_ref.holder.state.write().await;
        *state_guard = asset::AssetState::Loaded(resource.clone());
        drop(state_guard);
        Ok(resource)
    }
}

impl<A: Allocator + 'static> AssetManager<A> {
    /// Retrieve a buffer and load it automatically or wait until the buffer is available
    ///
    /// Only target format is used if we're loading an entirely new file
    pub async fn retrieve_buffer(
        &self,
        load_request: BufferRequest<A>,
    ) -> Result<Arc<resource::Buffer<A>>> {
        let mut allocator = self.allocator.clone();
        let res: Result<Arc<resource::Buffer<A>>> =
            match self.get::<asset::Buffer<A>>(Either::Right(&load_request.metadata)) {
                None => unimplemented!(),
                Some(slot) => {
                    let metadata = slot.holder.metadata.clone();
                    let slot = slot.holder.state.clone();
                    let state = slot.read().await;
                    if let asset::AssetState::Unloaded(_) = &*state {
                        drop(state);
                        let (sender, receiver) =
                            tokio::sync::watch::channel::<Option<Arc<resource::Buffer<A>>>>(None);
                        {
                            let mut slot_write_guard = slot.write().await;
                            *slot_write_guard = asset::AssetState::Loading(receiver)
                        }

                        let mut chunk_buffer =
                            resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                                device: self.allocator.device(),
                                allocator: &mut allocator,
                                size: load_request.chunk_size as vk::DeviceSize,
                                memory_type: MemoryLocation::CpuToGpu,
                                usage_flags: vk::BufferUsageFlags::TRANSFER_DST
                                    | vk::BufferUsageFlags::TRANSFER_SRC,
                            })?;
                        let buffer =
                            resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                                device: self.allocator.device(),
                                allocator: &mut allocator,
                                size: (metadata.element_format.size() * metadata.element_count)
                                    as vk::DeviceSize,
                                memory_type: MemoryLocation::GpuOnly,
                                usage_flags: load_request.buffer_usage
                                    | vk::BufferUsageFlags::TRANSFER_DST,
                            })?;

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
                            if let asset::AssetState::Loading(_) = &*slot_write_guard {
                                sender.send(Some(buffer.clone()))?;
                            }
                            *slot_write_guard = asset::AssetState::Loaded(buffer.clone());
                        }
                        Ok(buffer)
                    } else if let asset::AssetState::Loading(notify) = &*state {
                        let mut notify = notify.clone();
                        drop(state);
                        notify.changed().await?;
                        let x = Ok(notify.borrow().clone().unwrap().clone());
                        x
                    } else if let asset::AssetState::Loaded(buffer) = &*state {
                        Ok(buffer.clone())
                    } else if let asset::AssetState::Unloading(buffer) = &*state {
                        let buffer = buffer.clone();
                        drop(state);
                        let mut slot_write_guard = slot.write().await;
                        if let Some(buffer) = buffer.upgrade() {
                            *slot_write_guard = asset::AssetState::Loaded(buffer.clone());
                            Ok(buffer.clone())
                        } else {
                            *slot_write_guard = asset::AssetState::Unloaded(metadata.clone());
                            Ok(self.retrieve_buffer(load_request).await?)
                        }
                    } else {
                        unimplemented!()
                    }
                }
            };
        res
    }
}
