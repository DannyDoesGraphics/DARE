use crate::resource::traits::Resource;
/// # Virtual resources
use std::any::{Any, TypeId};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;

/// Represents a virtual_resources resource
#[derive(Debug, Clone)]
pub struct VirtualResource {
    pub uid: u64,
    pub gen: u64,
    pub drop_send: Option<crossbeam_channel::Sender<VirtualResource>>,
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
impl Drop for VirtualResource {
    fn drop(&mut self) {
        if let Some(drop_send) = &self.drop_send {
            drop_send
                .send(Self {
                    uid: self.uid,
                    gen: self.gen,
                    drop_send: None,
                    type_id: self.type_id,
                })
                .unwrap();
        }
    }
}
impl VirtualResource {
    /// If a virtual resource will destroy the physical resource upon being, dropped, this allows
    /// you to downgrade the virtual resource
    pub fn downgrade(&self) -> Self {
        Self {
            uid: self.uid,
            gen: self.gen,
            drop_send: None,
            type_id: self.type_id,
        }
    }
}
