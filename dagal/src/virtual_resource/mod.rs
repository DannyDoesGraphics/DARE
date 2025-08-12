/// # Virtual resources
use std::any::TypeId;
use std::hash::{Hash, Hasher};

/// Represents a virtual_resources resource
#[derive(Debug, Clone, Copy)]
pub struct VirtualResource {
    pub uid: u64,
    pub generation: u32,
    pub type_id: TypeId,
}
impl PartialEq for VirtualResource {
    fn eq(&self, other: &Self) -> bool {
        self.uid == other.uid
            && self.generation == other.generation
            && self.type_id == other.type_id
    }
}
impl Eq for VirtualResource {}
impl Hash for VirtualResource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uid.hash(state);
        self.generation.hash(state);
        self.type_id.hash(state);
    }
}
