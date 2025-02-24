use std::any::TypeId;
use std::collections::HashMap;
use dagal::resource::traits::Resource;
use dare_containers::prelude as containers;
use crate::asset2::prelude::AssetHandle;
use crate::asset2::traits::Asset;
use crate::render2::render_assets::traits::MetaDataRenderAsset;

pub fn slot_to_virtual_handle<T: 'static>(slot: containers::Slot<T>) -> dagal::resource::VirtualResource {
    dagal::resource::VirtualResource {
        uid: slot.id(),
        gen: slot.generation(),
        type_id: TypeId::of::<T>(),
    }
}

/// Storage for physical resources and used to allocate out [`dagal::resource::VirtualResource`]
pub struct PhysicalResourceStorage<T: MetaDataRenderAsset> {
    pub slot: containers::SlotMap<Option<T::Loaded>>,
    /// Map asset handles to virtual resource handles
    pub asset_mapping: HashMap<AssetHandle<T::Asset>, containers::Slot<Option<T::Loaded>>>,
}
impl<T: MetaDataRenderAsset> Default for PhysicalResourceStorage<T> {
    fn default() -> Self {
        Self {
            slot: Default::default(),
            asset_mapping: Default::default(),
        }
    }
}

impl<T: MetaDataRenderAsset> PhysicalResourceStorage<T> {
    /// Allocate a virtual resource handle with no physical resource backing
    pub fn get_virtual_handle(&mut self) -> dagal::resource::VirtualResource {
        let slot = self.slot.insert(None);
        let virtual_resource = slot_to_virtual_handle(slot);
        virtual_resource
    }

    /// Insert a physical resource to back a virtual resource
    pub fn alias(&mut self, virtual_resource: dagal::resource::VirtualResource, physical_resource: T::Loaded) -> Option<T::Loaded> {
        self.slot.get_mut(containers::Slot::new(virtual_resource.uid, virtual_resource.gen)).map(|option| {
            option.replace(physical_resource)
        }).flatten()
    }

    /// Insert a physical resource to back a new virtual resource
    pub fn insert_physical(&mut self, physical_resource: T::Loaded) -> dagal::resource::VirtualResource {
        let virt = self.get_virtual_handle();
        self.alias(virt, physical_resource);
        virt
    }

    /// Attempt to resolve a virtual resource
    pub fn resolve(&self, virtual_resource: dagal::resource::VirtualResource) -> Option<&T::Loaded> {
        self.slot.get(containers::Slot::new(virtual_resource.uid, virtual_resource.gen))
            .map(|option| option.as_ref())
            .flatten()
    }
}