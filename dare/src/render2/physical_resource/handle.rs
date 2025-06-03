use crate::util::either::Either;
use std::any::TypeId;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

/// Responsible to dealing with dropping of virtual resources
#[derive(Debug, Clone)]
struct VirtualResourceDrop {
    /// Internal weak reference
    weak: VirtualResource,
    /// Send to a channel to indicate drop
    send: crossbeam_channel::Sender<VirtualResource>,
}
impl Drop for VirtualResourceDrop {
    fn drop(&mut self) {
        // SAFETY: it is fine if we fail to indicate a resource needs to be dropped
        unsafe {
            self.send.send(self.weak.clone()).unwrap_or({});
        }
    }
}
impl PartialEq for VirtualResourceDrop {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for VirtualResourceDrop {}
impl Hash for VirtualResourceDrop {
    fn hash<H: Hasher>(&self, hash: &mut H) {
        self.weak.hash(hash);
    }
}

/// Internalized virtual resource handles
#[derive(Debug, Clone)]
pub struct VirtualResource {
    pub uid: u64,
    pub generation: u64,
    /// determines if current handle is considered to be a strong handle and should ref count
    pub ref_count: Option<Either<Weak<VirtualResourceDrop>, Arc<VirtualResourceDrop>>>,
    pub type_id: TypeId,
}
impl Hash for VirtualResource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uid.hash(state);
        self.generation.hash(state);
        self.type_id.hash(state);
    }
}
impl PartialEq for VirtualResource {
    fn eq(&self, other: &Self) -> bool {
        self.uid == other.uid
            && self.generation == other.generation
            && self.type_id == other.type_id
    }
}
impl Eq for VirtualResource {}

impl VirtualResource {
    pub fn new(
        uid: u64,
        generation: u64,
        send: Option<crossbeam_channel::Sender<VirtualResource>>,
        type_id: TypeId,
    ) -> Self {
        Self {
            uid,
            generation,
            ref_count: send.map(|send| {
                Either::Right(Arc::new(VirtualResourceDrop {
                    weak: VirtualResource {
                        uid,
                        generation,
                        ref_count: None,
                        type_id,
                    },
                    send,
                }))
            }),
            type_id,
        }
    }

    pub fn get_uid(&self) -> u64 {
        self.uid
    }
    pub fn get_gen(&self) -> u64 {
        self.generation
    }
    pub fn get_type_id(&self) -> TypeId {
        self.type_id
    }

    /// Downgrade a virtual resource to not ref count
    pub fn downgrade(&self) -> Self {
        Self {
            uid: self.uid,
            generation: self.generation,
            ref_count: self.ref_count.as_ref().map(|either| match either {
                Either::Left(weak) => Either::Left(weak.clone()),
                Either::Right(strong) => Either::Left(Arc::downgrade(strong)),
            }),
            type_id: self.type_id,
        }
    }

    /// Update a virtual resource to begin ref counting
    pub fn upgrade(&self) -> Option<Self> {
        self.ref_count
            .clone()
            .map(|ref_count| {
                let ref_count = match ref_count {
                    Either::Left(weak) => {
                        let v = weak.upgrade()?;
                        Some(Either::Right(v))
                    }
                    Either::Right(strong) => Some(Either::Right(strong)),
                };
                Some(Self {
                    uid: self.uid,
                    generation: self.generation,
                    ref_count,
                    type_id: self.type_id,
                })
            })
            .flatten()
    }
}
