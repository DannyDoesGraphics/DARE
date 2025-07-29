/// # Virtual resources
use std::any::{Any, TypeId};
use std::hash::{Hash, Hasher};

/// Represents a virtual_resources resource
#[derive(Debug, Clone)]
pub struct VirtualResource {
    pub uid: u64,
    pub gen: u64,
    pub type_id: TypeId,
}
impl PartialEq for VirtualResource {
    fn eq(&self, other: &Self) -> bool {
        self.uid == other.uid && self.gen == other.gen && self.type_id == other.type_id
    }
}
impl Eq for VirtualResource {}
impl Hash for VirtualResource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uid.hash(state);
        self.gen.hash(state);
        self.type_id.hash(state);
    }
}
