use std::hash::{Hash, Hasher};

#[repr(C)]
#[derive(Clone, PartialEq, Debug)]
pub struct InstancedSurfacesInfo {
    surface: u64,
    instances: u64,
    transforms: Vec<[f32; 16]>,
}
impl Hash for InstancedSurfacesInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.surface.hash(state);
        self.instances.hash(state);
        for transform in self.transforms.iter() {
            for i in transform.iter() {
                i.to_bits().hash(state);
            }
        }
    }
}
