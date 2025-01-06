use bytemuck::{Pod, Zeroable};
use std::hash::{Hash, Hasher};

#[repr(C)]
#[derive(Clone, PartialEq, Debug)]
pub struct InstancedSurfacesInfo {
    pub surface: u64,
    pub material: u64,
    pub instances: u64,
    pub transformation_offset: u64,
}
unsafe impl Zeroable for InstancedSurfacesInfo {}

impl Copy for InstancedSurfacesInfo {}

unsafe impl Pod for InstancedSurfacesInfo {}

impl Hash for InstancedSurfacesInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.surface.hash(state);
        self.instances.hash(state);
    }
}