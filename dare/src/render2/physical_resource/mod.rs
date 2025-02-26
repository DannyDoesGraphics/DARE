use crate::asset2::prelude::AssetHandle;
use crate::asset2::traits::Asset;
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use dagal::resource::traits::Resource;
use dare_containers::prelude as containers;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;
pub mod gpu_stream;
pub use gpu_stream::*;

pub fn slot_to_virtual_handle<T: 'static>(
    slot: containers::Slot<T>,
    drop_send: Option<crossbeam_channel::Sender<dagal::resource::VirtualResource>>,
) -> dagal::resource::VirtualResource {
    dagal::resource::VirtualResource {
        uid: slot.id(),
        gen: slot.generation(),
        drop_send,
        type_id: TypeId::of::<T>(),
    }
}

/// Internalize deletion entry
struct DeletionSlot {
    lifetime: u32,
    current: u32,
    virtual_resource: Option<Arc<dagal::resource::VirtualResource>>,
}
impl DeletionSlot {
    /// Reset current lifetime
    pub fn reset(&mut self) {
        self.current = self.lifetime
    }
}

/// Storage for physical resources and used to allocate out [`dagal::resource::VirtualResource`]
///
/// # Error handling
/// When loading, if an asset fails to properly load, we will not record that it failed.
pub struct PhysicalResourceStorage<T: MetaDataRenderAsset> {
    pub slot: containers::SlotMap<Option<T::Loaded>>,
    /// Map asset handles to virtual resource handles
    pub asset_mapping: HashMap<AssetHandle<T::Asset>, dagal::resource::VirtualResource>,
    /// for asset loading
    loaded_send: crossbeam_channel::Sender<(dagal::resource::VirtualResource, T::Loaded)>,
    loaded_recv: crossbeam_channel::Receiver<(dagal::resource::VirtualResource, T::Loaded)>,
    /// for handle drops
    drop_send: crossbeam_channel::Sender<dagal::resource::VirtualResource>,
    drop_recv: crossbeam_channel::Receiver<dagal::resource::VirtualResource>,
    /// We can also perform deferred deletion strategies
    /// (current_lifetime, old_lifetime, reference)
    deferred_deletion: HashMap<dagal::resource::VirtualResource, DeletionSlot>,
}
impl<T: MetaDataRenderAsset> Default for PhysicalResourceStorage<T> {
    fn default() -> Self {
        let (loaded_send, loaded_recv) = crossbeam_channel::unbounded();
        let (drop_send, drop_recv) = crossbeam_channel::unbounded();
        Self {
            slot: Default::default(),
            asset_mapping: Default::default(),
            loaded_send,
            loaded_recv,
            drop_send,
            drop_recv,
            deferred_deletion: Default::default(),
        }
    }
}

impl<T: MetaDataRenderAsset> PhysicalResourceStorage<T> {
    /// Allocate a virtual resource handle with no physical resource backing
    pub fn get_virtual_handle(&mut self) -> dagal::resource::VirtualResource {
        let slot = self.slot.insert(None);
        slot_to_virtual_handle(slot, None)
    }

    pub fn get_deferred_virtual_handle(
        &mut self,
        lifetime: u32,
    ) -> Arc<dagal::resource::VirtualResource> {
        let slot = self.slot.insert(None);
        let virtual_resource = slot_to_virtual_handle(slot, Some(self.drop_send.clone()));
        self.deferred_deletion
            .entry(virtual_resource.downgrade())
            .or_insert(DeletionSlot {
                lifetime: 0,
                current: 0,
                virtual_resource: None,
            })
            .and_modify(|deletion_slot| {
                // reset lifetime if it exists
                deletion_slot.lifetime = lifetime;
                deletion_slot.reset();
            });
        Arc::new(virtual_resource)
    }

    /// Insert a physical resource to back a virtual resource
    pub fn alias(
        &mut self,
        virtual_resource: dagal::resource::VirtualResource,
        physical_resource: T::Loaded,
    ) -> Option<T::Loaded> {
        self.slot
            .get_mut(containers::Slot::new(
                virtual_resource.uid,
                virtual_resource.gen,
            ))
            .map(|option| option.replace(physical_resource))
            .flatten()
    }

    /// Alias an asset handle to a virtual resource
    pub fn asset_alias(
        &mut self,
        virtual_resource: &dagal::resource::VirtualResource,
        handle: AssetHandle<T::Asset>,
    ) -> Option<T::Loaded> {
        // reset counter (if it exists)
        self.asset_mapping
            .insert(handle, virtual_resource.downgrade())
    }

    /// Insert a deferred resource
    pub fn alias_deferred(
        &mut self,
        virtual_resource: dagal::resource::VirtualResource,
        physical_resource: T::Loaded,
    ) -> Option<T::Loaded> {
        self.slot
            .get_mut(containers::Slot::new(
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

    /// Insert a physical resource to back a new virtual resource
    pub fn insert_physical(
        &mut self,
        physical_resource: T::Loaded,
    ) -> dagal::resource::VirtualResource {
        let virt = self.get_virtual_handle();
        self.alias(virt.clone(), physical_resource);
        virt
    }

    /// Insert a deferred physical resource back to a new virtual resource
    pub fn insert_deferred_physical(
        &mut self,
        lifetime: u32,
        physical_resource: T::Loaded,
    ) -> Arc<dagal::resource::VirtualResource> {
        let virtual_handle = self.get_deferred_virtual_handle(lifetime);
        if lifetime > 0 {
            let mut deletion = self.deferred_deletion.get_mut(&virtual_handle).unwrap(); // unwrap should be *fine* here, since [`Self::get_deferred_virtual_handle`] properly sets up the deletion entry.
            deletion.reset();
            deletion.virtual_resource.replace(virtual_handle.clone());
        }
        self.alias(virtual_handle.downgrade(), physical_resource); // we don't need to do deferred for now...
        virtual_handle
    }

    /// Acquire a channel to notify physical resource storage of successful asset loading
    pub fn asset_loaded_queue(
        &self,
    ) -> crossbeam_channel::Sender<(dagal::resource::VirtualResource, T::Loaded)> {
        self.loaded_send.clone()
    }

    /// Attempt to resolve a virtual resource
    pub fn resolve(
        &mut self,
        virtual_resource: dagal::resource::VirtualResource,
    ) -> Option<&T::Loaded> {
        self.slot
            .get(containers::Slot::new(
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

    /// Process loaded assets
    pub fn update(&mut self) {
        // handle inserts
        for (virtual_resource, physical_resource) in self.loaded_recv.recv() {
            self.asset_alias(&virtual_resource, physical_resource);
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
        while let Ok((virtual_resource, asset)) = self.loaded_recv.recv() {
            // find each loaded, update virtual resource to reflect new asset
            self.slot
                .get_mut(containers::Slot::new(
                    virtual_resource.uid,
                    virtual_resource.gen,
                ))
                .unwrap()
                .replace(asset);
        }
        // remove dropped
        while let Ok(virtual_resource) = self.drop_recv.recv() {
            // drop the physical resource
            self.slot
                .get_mut(containers::Slot::new(
                    virtual_resource.uid,
                    virtual_resource.gen,
                ))
                .unwrap()
                .take();
        }
    }
}
