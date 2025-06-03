use crate::asset2::prelude::AssetHandle;
use crate::asset2::traits::Asset;
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use bevy_ecs::prelude::*;
use containers::DefaultSlot;
use dagal::allocators::Allocator;
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use dare_containers::prelude as containers;
use dare_containers::slot::{Slot, SlotWithGeneration};
use futures::TryFutureExt;
use std::any::TypeId;
use std::collections::HashMap;
use std::hash::Hash;

pub mod gpu_stream;
pub use gpu_stream::*;
pub mod handle;
pub mod render_buffer;
pub mod render_image;

use crate::asset2::loaders::MetaDataLoad;
use crate::asset2::prelude as asset;
use crate::asset2::server::AssetServer;
pub use handle::*;
pub use render_buffer::*;
pub use render_image::*;

pub fn slot_to_virtual_handle<T: 'static>(
    slot: DefaultSlot<T>,
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
    /// Represents a **temporary** unused state
    Empty,
    /// Load lock to prevent double loads
    Loading,
    /// Existing
    Some(T),
}
impl<T> PhysicalState<T> {
    pub fn is_some(&self) -> bool {
        matches!(self, PhysicalState::Some(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self, PhysicalState::Empty)
    }

    pub fn map<F: FnOnce(&T) -> A, A>(&self, f: F) -> Option<A> {
        match self {
            PhysicalState::Loading | PhysicalState::Empty => None,
            PhysicalState::Some(t) => Some(f(t)),
        }
    }

    pub fn replace(&mut self, val: T) -> Option<T> {
        match self {
            PhysicalState::Loading | PhysicalState::Empty => {
                *self = PhysicalState::Some(val);
                None
            }
            PhysicalState::Some(t) => Some(std::mem::replace(t, val)),
        }
    }

    pub fn as_ref(&self) -> Option<&T> {
        match self {
            PhysicalState::Some(t) => Some(t),
            _ => None,
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

    /// Allocate a virtual resource handle that is in the process of loading
    pub fn get_virtual_handle(&mut self, lifetime: Option<u32>) -> VirtualResource {
        let slot = self.slot.insert(PhysicalState::Loading);
        let virtual_resource = slot_to_virtual_handle(
            slot,
            match lifetime {
                Some(_) => Some(self.drop_send.clone()),
                None => None,
            },
        );
        match lifetime {
            None => virtual_resource,
            Some(lifetime) => {
                let deletion_slot = self
                    .deferred_deletion
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
            .get_mut(DefaultSlot::new(
                virtual_resource.uid,
                virtual_resource.generation,
            ))
            .and_then(|option| option.replace(physical_resource))
    }

    /// Alias an asset handle to a virtual resource
    pub fn asset_alias(
        &mut self,
        virtual_resource: &VirtualResource,
        handle: AssetHandle<T::Asset>,
    ) -> Option<VirtualResource> {
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
            .get_mut(DefaultSlot::new(
                virtual_resource.uid,
                virtual_resource.generation,
            ))
            .and_then(|option| {
                // reset lifetime
                if let Some(deferred) = self.deferred_deletion.get_mut(&virtual_resource) {
                    deferred.reset()
                }
                option.replace(physical_resource)
            })
    }
    /// Insert a deferred physical resource back to a new virtual resource
    pub fn insert_deferred(
        &mut self,
        lifetime: Option<u32>,
        physical_resource: T::Loaded,
    ) -> VirtualResource {
        let virtual_handle = self.get_virtual_handle(lifetime);
        if lifetime.map(|v| v > 0).unwrap_or(false) {
            let deletion = self.deferred_deletion.get_mut(&virtual_handle).unwrap(); // unwrap should be *fine* here, since [`Self::get_deferred_virtual_handle`] properly sets up the deletion entry.
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
            .get(DefaultSlot::new(
                virtual_resource.uid,
                virtual_resource.generation,
            ))
            .and_then(|option| match option.as_ref() {
                None => None,
                Some(r) => {
                    // keep alive
                    if let Some(deferred) = self.deferred_deletion.get_mut(virtual_resource) {
                        deferred.reset()
                    }
                    Some(r)
                }
            })
    }

    /// Resolve using an asset instead of virtual resource
    pub fn resolve_asset(&mut self, asset_handle: &AssetHandle<T::Asset>) -> Option<&T::Loaded> {
        match self.asset_mapping.get(asset_handle).cloned() {
            None => None,
            Some(v) => self.resolve(&v),
        }
    }

    /// Resolve for a virtual resource handle that is mapped to an [`AssetHandle`]
    pub fn resolve_virtual_resource(
        &self,
        virtual_resource: &AssetHandle<T::Asset>,
    ) -> Option<&VirtualResource> {
        self.asset_mapping.get(virtual_resource)
    }

    /// Process loaded assets
    pub fn update(&mut self) {
        // handle inserts
        while let Ok((virtual_resource, physical_resource)) = self.loaded_recv.try_recv() {
            // find each loaded, update virtual resource to reflect new asset
            let old = self
                .slot
                .get_mut(DefaultSlot::new(
                    virtual_resource.uid,
                    virtual_resource.generation,
                ))
                .unwrap();
            if !old.is_some() {
                old.replace(physical_resource);
            } else {
                tracing::error!("Attempted to replace an already existing resource");
            }
        }
        // decrement lifetimes
        for deferred in self
            .deferred_deletion
            .values_mut()
            .filter(|v| v.lifetime != 0 && v.virtual_resource.is_some())
        {
            deferred.current = deferred.current.saturating_sub(1);
            if deferred.current == 0 {
                // lifetime has reached zero, get rid of the call strong handle we have on it
                deferred.virtual_resource.take();
            }
        }
        // remove dropped
        while let Ok(virtual_resource) = self.drop_recv.try_recv() {
            if let Ok(t) = self.slot.remove(DefaultSlot::new(
                virtual_resource.uid,
                virtual_resource.generation,
            )) {
                self.asset_mapping_reverse
                    .remove(&virtual_resource)
                    .map(|vr| self.asset_mapping.remove(&vr));
                drop(t);
            }
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
        let slot = DefaultSlot::new(virtual_handle.uid, virtual_handle.generation);
        let is_unloaded = self
            .slot
            .get(slot.clone())
            .map(|state| state.is_none())
            .unwrap_or(false);
        if is_unloaded {
            let virtual_handle = virtual_handle.clone();
            // get deferred deletion and reset
            if let Some(deferred_deletion) = self.deferred_deletion.get_mut(&virtual_handle) {
                deferred_deletion.reset();
            }
            // apply lock
            if let Some(slot) = self.slot.get_mut(slot) {
                *slot = PhysicalState::Loading;
            }
            if let Some(metadata) = self.asset_server.get_metadata(&asset_handle) {
                tokio::task::spawn(async move {
                    T::load_asset(metadata, prepare_info, load_info)
                        .and_then(|v| async move {
                            if finished_queue.send((virtual_handle.clone(), v)).is_err() {
                                tracing::error!(
                                    "Physical resource storage failed to send loaded resource"
                                );
                            };
                            Ok(())
                        })
                        .await
                });
            }
        }
    }

    /// Similarly to `load_asset_handle` however, we will insert the resource and perform a load if it does not exist
    pub fn load_or_create_asset_handle(
        &mut self,
        asset_handle: AssetHandle<T::Asset>,
        prepare_info: T::PrepareInfo,
        load_info: <<T::Asset as Asset>::Metadata as MetaDataLoad>::LoadInfo<'static>,
        lifetime: u32,
    ) {
        let asset_handle = asset_handle.downgrade();
        if self
            .asset_mapping
            .get(&asset_handle)
            .map(|vr| vr.upgrade().is_none())
            .unwrap_or(true)
        {
            let virtual_resource = {
                let slot = self.slot.insert(PhysicalState::Empty);
                let virtual_resource = slot_to_virtual_handle(slot, Some(self.drop_send.clone()));
                let deletion_slot = self
                    .deferred_deletion
                    .entry(virtual_resource.downgrade())
                    .or_default();
                deletion_slot.virtual_resource = Some(virtual_resource.clone());
                deletion_slot.lifetime = lifetime;
                virtual_resource
            }
            .downgrade();
            // apply mapping
            self.asset_mapping_reverse
                .insert(virtual_resource.clone(), asset_handle.clone());
            self.asset_mapping
                .insert(asset_handle.clone(), virtual_resource);
        }
        self.load_asset_handle(&asset_handle, prepare_info, load_info);
    }
}

impl<A: Allocator + 'static> PhysicalResourceStorage<RenderBuffer<A>> {
    pub fn get_bda(
        &mut self,
        asset_handle: &AssetHandle<asset::assets::Buffer>,
    ) -> Option<vk::DeviceAddress> {
        if let Some(vr) = self
            .asset_mapping
            .get(asset_handle)
            .and_then(|vr| vr.upgrade())
        {
            self.slot
                .get(DefaultSlot::new(vr.uid, vr.generation))?
                .as_ref()
                .map(|buf| {
                    if let Some(deferred) = self.deferred_deletion.get_mut(&vr) {
                        deferred.reset()
                    }
                    buf.address()
                })
        } else {
            None
        }
    }
}

/// Similar to a [`PhysicalResourceStorage`] however with the difference that we maintain hash
/// uniqueness per asset create information.
///
/// This is primarily useful if we have certain objects such as Samplers which are universal across
/// images
pub struct PhysicalResourceHashMap<
    T: MetaDataRenderAsset<Loaded = L, Asset = A>,
    A: Hash + PartialEq + Eq + Clone,
    L: Send + Resource,
> {
    resource: PhysicalResourceStorage<T>,
    map: HashMap<A, VirtualResource>,
}

impl<
    T: MetaDataRenderAsset<Loaded = L, Asset = A>,
    A: Hash + PartialEq + Eq + Clone,
    L: Send + Resource,
> PhysicalResourceHashMap<T, A, L>
{
    /// Create a new hashmap
    pub fn new(asset_server: AssetServer) -> Self {
        Self {
            resource: PhysicalResourceStorage::new(asset_server),
            map: HashMap::new(),
        }
    }

    /// Attempts to retrieve using create info, if not, force a load to occur
    pub fn retrieve(&mut self, ci: A) -> Option<&T::Loaded> {
        // do a check if the virtual resource exists
        let vr_option = self.map.get(&ci).cloned();
        let is_vr_option = vr_option.is_some();
        if let Some(vr) = vr_option {
            // mapping found, attempt to retrieve it
            if self.resource.resolve(&vr).is_some() {
                return self.resource.resolve(&vr);
            }
            // if not, go try to retrieve again
        }
        // make a new resource, since we cannot resolve it
        let virtual_resource = self.resource.get_virtual_handle(Some(0));
        // if mapping doesn't exist yet, map it
        if !is_vr_option {
            self.map.insert(ci, virtual_resource.clone());
        }
        //self.resource.alias(&virtual_resource, loaded);
        self.resource.resolve(&virtual_resource)
    }

    /// Retrieve the underlying [`PhysicalResourceStorage`]
    pub fn physical_resource_storage(&self) -> &PhysicalResourceStorage<T> {
        &self.resource
    }
}
