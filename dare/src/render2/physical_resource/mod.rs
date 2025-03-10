use crate::asset2::prelude::{AssetHandle, AssetMetadata};
use crate::asset2::traits::Asset;
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use dagal::resource::traits::Resource;
use dare_containers::prelude as containers;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use bevy_ecs::query::Has;
use futures::TryFutureExt;

pub mod gpu_stream;
pub use gpu_stream::*;
pub mod handle;
pub use handle::*;
use crate::asset2::loaders::MetaDataLoad;
use crate::asset2::server::asset_info::AssetInfo;
use crate::asset2::server::AssetServer;

pub fn slot_to_virtual_handle<T: 'static>(
    slot: containers::Slot<T>,
    drop_send: Option<crossbeam_channel::Sender<VirtualResource>>,
) -> VirtualResource {
    VirtualResource::new(slot.id(), slot.generation(), drop_send, TypeId::of::<T>())
}

/// Internalize deletion entry
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

/// Storage for physical resources and used to allocate out [`VirtualResource`]
///
/// # Error handling
/// When loading, if an asset fails to properly load, we will not record that it failed.
pub struct PhysicalResourceStorage<T: MetaDataRenderAsset> {
    pub asset_server: AssetServer,
    pub slot: containers::SlotMap<Option<T::Loaded>>,
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
    fn new(asset_server: AssetServer) -> Self {
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
    pub fn get_virtual_handle(&mut self) -> VirtualResource {
        let slot = self.slot.insert(None);
        slot_to_virtual_handle(slot, None)
    }

    pub fn get_deferred_virtual_handle(&mut self, lifetime: u32) -> VirtualResource {
        let slot = self.slot.insert(None);
        let virtual_resource = slot_to_virtual_handle(slot, Some(self.drop_send.clone()));
        let mut deletion_slot = self
            .deferred_deletion
            .entry(virtual_resource.downgrade())
            .or_insert(DeletionSlot {
                lifetime: 0,
                current: 0,
                virtual_resource: None,
            });
        deletion_slot.lifetime = lifetime;
        virtual_resource
    }

    /// Insert a physical resource to back a virtual resource
    pub fn alias(
        &mut self,
        virtual_resource: &VirtualResource,
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
        virtual_resource: &VirtualResource,
        handle: AssetHandle<T::Asset>,
    ) -> Option<VirtualResource> {
        // reset counter (if it exists)
        self.asset_mapping_reverse
            .insert(
                virtual_resource.downgrade(),
                handle.clone().downgrade(),
            );
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
    pub fn insert_physical(&mut self, physical_resource: T::Loaded) -> VirtualResource {
        let virt = self.get_virtual_handle();
        self.alias(&virt, physical_resource);
        virt
    }

    /// Insert a deferred physical resource back to a new virtual resource
    pub fn insert_deferred_physical(
        &mut self,
        lifetime: u32,
        physical_resource: T::Loaded,
    ) -> VirtualResource {
        let virtual_handle = self.get_deferred_virtual_handle(lifetime);
        if lifetime > 0 {
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
    pub fn resolve(&mut self, virtual_resource: VirtualResource) -> Option<&T::Loaded> {
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
            self.alias(&virtual_resource, physical_resource);
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

    /// If an asset has an associated asset handle already, we can use that to automatically load it
    /// from a different thread
    pub fn load_asset_handle(&mut self, virtual_resource: VirtualResource, prepare_info: T::PrepareInfo, load_info: <<T::Asset as Asset>::Metadata as MetaDataLoad>::LoadInfo<'static> ) {
        let finished_queue = self.loaded_send.clone();
        self.asset_mapping_reverse.get(&virtual_resource).map(|asset_handle| {
                self.asset_server.get_metadata(&asset_handle).map(|metadata| {
                    tokio::spawn(async move {
                        T::load_asset(
                            metadata,
                            prepare_info,
                            load_info,
                        ).and_then(|v| async move {
                            if let Err(e) = finished_queue.send((virtual_resource, v)) {
                                tracing::error!("Physical resource storage failed to send loaded resource");
                            };
                            Ok(())
                        }).await
                    });
                });
        });
    }
}
