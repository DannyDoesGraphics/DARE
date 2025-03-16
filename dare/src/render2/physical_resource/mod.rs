use crate::asset2::prelude::{AssetHandle, AssetMetadata};
use crate::asset2::traits::Asset;
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use bevy_ecs::prelude::*;
use dagal::resource::traits::Resource;
use dare_containers::prelude as containers;
use futures::TryFutureExt;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use dagal::allocators::Allocator;
use dagal::ash::vk;
use containers::Slot;

pub mod gpu_stream;
pub use gpu_stream::*;
pub mod handle;
pub mod render_buffer;
pub mod render_image;

pub use render_buffer::*;
pub use render_image::*;
use crate::asset2::loaders::MetaDataLoad;
use crate::asset2::server::asset_info::AssetInfo;
use crate::asset2::server::AssetServer;
pub use handle::*;
use crate::asset2::prelude as asset;

pub fn slot_to_virtual_handle<T: 'static>(
    slot: Slot<T>,
    drop_send: Option<crossbeam_channel::Sender<VirtualResource>>,
) -> VirtualResource {
    VirtualResource::new(slot.id(), slot.generation(), drop_send, TypeId::of::<T>())
}

/// Internalize deletion entry
#[derive(Default)]
struct DeletionSlot {
    lifetime: u32,
    current: u32,
    virtual_resource: Option<VirtualResource>,
}
impl DeletionSlot {
    /// Reset current lifetime
    pub fn reset(&mut self) {
        self.current = self.lifetime
    }
}

enum PhysicalState<T> {
    None,
    Loading,
    Some(T)
}
impl<T> PhysicalState<T> {
    pub fn is_some(&self) -> bool {
        match self {
            PhysicalState::Some(_) => true,
            _ => false
        }
    }

    pub fn is_none(&self) -> bool {
        match self {
            PhysicalState::None => true,
            _ => false
        }
    }

    pub fn map<F: FnOnce(&T) -> A, A>(&self, f: F) -> Option<A> {
        match self {
            PhysicalState::None => None,
            PhysicalState::Loading => None,
            PhysicalState::Some(t) => Some(f(t))
        }
    }

    pub fn replace(&mut self, mut val: T) -> Option<T> {
        match self {
            PhysicalState::None | PhysicalState::Loading => {
                *self = PhysicalState::Some(val);
                None
            },
            PhysicalState::Some(t) => Some(std::mem::replace(t, val)),
        }
    }

    pub fn take(&mut self) -> Option<T> {
        let mut state = PhysicalState::None;
        std::mem::swap(&mut state, self);
        match state {
            PhysicalState::Some(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_ref(&self) -> Option<&T> {
        match self {
            PhysicalState::Some(t) => Some(t),
            _ => None
        }
    }
}

/// Storage for physical resources and used to allocate out [`VirtualResource`]
///
/// # Error handling
/// When loading, if an asset fails to properly load, we will not record that it failed.
#[derive(Resource)]
pub struct PhysicalResourceStorage<T: MetaDataRenderAsset> {
    pub asset_server: AssetServer,
    pub slot: containers::SlotMap<PhysicalState<T::Loaded>>,
    /// Map asset handles to virtual resource handles
    asset_mapping: HashMap<AssetHandle<T::Asset>, VirtualResource>,
    asset_mapping_reverse: HashMap<VirtualResource, AssetHandle<T::Asset>>,
    /// for asset loading
    loaded_send: crossbeam_channel::Sender<(VirtualResource, T::Loaded)>,
    loaded_recv: crossbeam_channel::Receiver<(VirtualResource, T::Loaded)>,
    /// for handle drops
    drop_send: crossbeam_channel::Sender<VirtualResource>,
    drop_recv: crossbeam_channel::Receiver<VirtualResource>,
    /// We can also perform deferred deletion strategies
    /// (current_lifetime, old_lifetime, reference)
    deferred_deletion: HashMap<VirtualResource, DeletionSlot>,
}

impl<T: MetaDataRenderAsset> PhysicalResourceStorage<T> {
    pub fn new(asset_server: AssetServer) -> Self {
        let (loaded_send, loaded_recv) = crossbeam_channel::unbounded();
        let (drop_send, drop_recv) = crossbeam_channel::unbounded();
        Self {
            asset_server,
            slot: Default::default(),
            asset_mapping: Default::default(),
            asset_mapping_reverse: Default::default(),
            loaded_send,
            loaded_recv,
            drop_send,
            drop_recv,
            deferred_deletion: Default::default(),
        }
    }

    /// Allocate a virtual resource handle with no physical resource backing
    pub fn get_virtual_handle(&mut self, lifetime: Option<u32>) -> VirtualResource {
        let slot = self.slot.insert(PhysicalState::None);
        let virtual_resource = slot_to_virtual_handle(slot, match lifetime {
            Some(_) => Some(self.drop_send.clone()),
            None => None
        });
        match lifetime {
            None => {
                virtual_resource
            }
            Some(lifetime) => {
                let mut deletion_slot =
                self.deferred_deletion
                    .entry(virtual_resource.clone())
                    .or_default();
                deletion_slot.lifetime = lifetime;
                virtual_resource
            }
        }
    }

    /// Insert a physical resource to back a virtual resource
    pub fn alias(
        &mut self,
        virtual_resource: &VirtualResource,
        physical_resource: T::Loaded,
    ) -> Option<T::Loaded> {
        self.slot
            .get_mut(Slot::new(
                virtual_resource.uid,
                virtual_resource.gen,
            ))
            .map(|option| {
                option.replace(physical_resource)
            })
            .flatten()
    }

    /// Alias an asset handle to a virtual resource
    pub fn asset_alias(
        &mut self,
        virtual_resource: &VirtualResource,
        handle: AssetHandle<T::Asset>,
    ) -> Option<VirtualResource> {
        // reset counter (if it exists)
        self.asset_mapping_reverse
            .insert(virtual_resource.downgrade(), handle.clone().downgrade());
        self.asset_mapping
            .insert(handle.downgrade(), virtual_resource.downgrade())
    }

    /// Insert a deferred resource
    pub fn alias_deferred(
        &mut self,
        virtual_resource: VirtualResource,
        physical_resource: T::Loaded,
    ) -> Option<T::Loaded> {
        self.slot
            .get_mut(Slot::new(
                virtual_resource.uid,
                virtual_resource.gen,
            ))
            .map(|option| {
                // reset lifetime
                self.deferred_deletion
                    .get_mut(&virtual_resource)
                    .map(|deferred| deferred.reset());
                option.replace(physical_resource)
            })
            .flatten()
    }
    /// Insert a deferred physical resource back to a new virtual resource
    pub fn insert_deferred(
        &mut self,
        lifetime: Option<u32>,
        physical_resource: T::Loaded,
    ) -> VirtualResource {
        let virtual_handle = self.get_virtual_handle(lifetime);
        if lifetime.map(|v| v > 0).unwrap_or(false) {
            let mut deletion = self.deferred_deletion.get_mut(&virtual_handle).unwrap(); // unwrap should be *fine* here, since [`Self::get_deferred_virtual_handle`] properly sets up the deletion entry.
            deletion.reset();
            deletion.virtual_resource.replace(virtual_handle.clone());
        }
        self.alias(&virtual_handle, physical_resource); // we don't need to do deferred for now...
        virtual_handle
    }

    /// Acquire a channel to notify physical resource storage of successful asset loading
    pub fn asset_loaded_queue(&self) -> crossbeam_channel::Sender<(VirtualResource, T::Loaded)> {
        self.loaded_send.clone()
    }

    /// Attempt to resolve a virtual resource
    pub fn resolve(&mut self, virtual_resource: &VirtualResource) -> Option<&T::Loaded> {
        self.slot
            .get(Slot::new(
                virtual_resource.uid,
                virtual_resource.gen,
            ))
            .map(|option| match option.as_ref() {
                None => None,
                Some(r) => {
                    self.deferred_deletion
                        .get_mut(&virtual_resource)
                        .map(|deferred| deferred.reset());
                    Some(r)
                }
            })
            .flatten()
    }

    /// Resolve using an asset instead of virtual resource
    pub fn resolve_asset(&mut self, asset_handle: &AssetHandle<T::Asset>) -> Option<&T::Loaded> {
        match self.asset_mapping.get(asset_handle).cloned() {
            None => {
                None
            }
            Some(v) => {
                self.resolve(&v)
            }
        }
    }

    /// Resolve for a virtual resource handle that is mapped to an [`AssetHandle`]
    pub fn resolve_virtual_resource(&self, virtual_resource: &AssetHandle<T::Asset>) -> Option<&VirtualResource> {
        self
            .asset_mapping
            .get(virtual_resource)
    }

    /// Process loaded assets
    pub fn update(&mut self) {
        // handle inserts
        while let Ok((virtual_resource, physical_resource)) = self.loaded_recv.try_recv() {
            // find each loaded, update virtual resource to reflect new asset
            self.slot
                .get_mut(Slot::new(
                    virtual_resource.uid,
                    virtual_resource.gen,
                ))
                .unwrap()
                .replace(physical_resource);
        }
        // decrement lifetimes
        for deferred in self
            .deferred_deletion
            .values_mut()
            .filter(|v| v.lifetime != 0)
        {
            let _ = deferred.current.saturating_sub(1);
            if deferred.current == 0 {
                // lifetime has reached zero, get rid of it
                deferred.virtual_resource.take();
            }
        }
        // remove dropped
        while let Ok(virtual_resource) = self.drop_recv.try_recv() {
            self.slot.get_mut(Slot::new(
                virtual_resource.uid,
                virtual_resource.gen,
            )).map(|physical_state| {
                physical_state.take();
            });
        }
    }

    /// If an asset has an associated asset handle already, we can use that to automatically load it
    /// from a different thread
    pub fn load_asset_handle(
        &mut self,
        asset_handle: &AssetHandle<T::Asset>,
        prepare_info: T::PrepareInfo,
        load_info: <<T::Asset as Asset>::Metadata as MetaDataLoad>::LoadInfo<'static>,
    ) {
        let finished_queue = self.loaded_send.clone();
        let virtual_handle = self.asset_mapping.get(asset_handle).unwrap();
        let slot = Slot::new(
            virtual_handle.uid,
            virtual_handle.gen,
        );
        let is_unloaded = self.slot.get(slot.clone()).map(|state| state.is_none()).unwrap_or(false);
        if is_unloaded {
            let virtual_handle = virtual_handle.clone();
            self.slot.get_mut(slot.clone()).map(|state| *state = PhysicalState::Loading);
                self.asset_server.get_metadata(&asset_handle).map(|metadata| {
                    let id = asset_handle.id();
                    tokio::task::spawn(async move {
                        T::load_asset(
                            metadata,
                            prepare_info,
                            load_info,
                        ).and_then(|v| async move {
                            if let Err(_) = finished_queue.send((virtual_handle.clone(), v)) {
                                tracing::error!("Physical resource storage failed to send loaded resource");
                            };
                            Ok(())
                        }).await
                    });
                });
        }
    }

    /// Similar to `load_asset_handle` however, we will insert the resource and perform a load if it does not exist
    pub fn load_or_create(
        &mut self,
        asset_handle: AssetHandle<T::Asset>,
        prepare_info: T::PrepareInfo,
        load_info: <<T::Asset as Asset>::Metadata as MetaDataLoad>::LoadInfo<'static>,
        lifetime: u32,
    ) {
        let asset_handle = asset_handle.downgrade();
        if !self.asset_mapping.contains_key(&asset_handle) {
                let virtual_resource = {
                    let slot = self.slot.insert(PhysicalState::None);
                    let virtual_resource = slot_to_virtual_handle(slot, Some(self.drop_send.clone()));
                    let mut deletion_slot =
                        self.deferred_deletion
                            .entry(virtual_resource.downgrade())
                            .or_default();
                    deletion_slot.virtual_resource = Some(virtual_resource.clone());
                    deletion_slot.lifetime = lifetime;
                    virtual_resource
                }.downgrade();
                // apply mapping
                self.asset_mapping_reverse.insert(virtual_resource.clone(), asset_handle.clone());
                self.asset_mapping.insert(
                    asset_handle.clone(),
                    virtual_resource
                );
        }
        self.load_asset_handle(&asset_handle, prepare_info, load_info);
    }

    pub fn keep_deferred_alive(&mut self, virtual_resource: &VirtualResource) {
        self.deferred_deletion.get_mut(virtual_resource)
            .map(|deferred| deferred.reset());
    }
}

impl<A: Allocator + 'static> PhysicalResourceStorage<RenderBuffer<A>> {
    pub fn get_bda(&mut self, asset_handle: &AssetHandle<asset::assets::Buffer>) -> Option<vk::DeviceAddress> {
        if let Some(vr) = self.asset_mapping.get(asset_handle) {
            self.slot.get(
                Slot::new(
                    vr.uid,
                    vr.gen
                )
            )?
                .as_ref()
                .map(|buf| {
                    self.deferred_deletion.get_mut(vr)
                        .map(|deferred| deferred.reset());
                    buf.address()
                })
        } else {
            None
        }
    }
}