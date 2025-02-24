use std::any::TypeId;
use std::collections::HashMap;
use dagal::resource::traits::Resource;
use dare_containers::prelude as containers;
use crate::asset2::prelude::AssetHandle;
use crate::asset2::traits::Asset;
use crate::render2::render_assets::traits::MetaDataRenderAsset;

pub fn slot_to_virtual_handle<T>(slot: containers::Slot<T>) -> dagal::resource::VirtualResource {
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

    }
}