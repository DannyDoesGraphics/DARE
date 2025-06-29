use crate::util::either::Either;
use dare_containers::slot::{Slot, SlotWithGeneration};
use std::any::TypeId;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

/// Responsible to dealing with dropping of virtual resources
#[derive(Debug, Clone)]
pub(crate) struct VirtualResourceDrop {
    /// Internal weak reference
    weak: VirtualResource,
    /// Send to a channel to indicate drop
    send: crossbeam_channel::Sender<VirtualResource>,
}
impl Drop for VirtualResourceDrop {
    fn drop(&mut self) {
        // It is fine if we fail to indicate a resource needs to be dropped
        let _ = self.send.send(self.weak.clone());
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
#[derive(Clone)]
pub struct VirtualResource {
    pub uid: u64,
    pub generation: u64,
    /// determines if current handle is considered to be a strong handle and should ref count
    pub(crate) ref_count: Option<Either<Weak<VirtualResourceDrop>, Arc<VirtualResourceDrop>>>,
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
    /// Set the drop semantics for this virtual resource
    pub fn set_drop_semantics(&mut self, send: Option<crossbeam_channel::Sender<VirtualResource>>) {
        if let Some(send) = send {
            self.ref_count = Some(Either::Right(Arc::new(VirtualResourceDrop {
                weak: VirtualResource {
                    uid: self.uid,
                    generation: self.generation,
                    ref_count: None,
                    type_id: self.type_id,
                },
                send,
            })));
        } else {
            self.ref_count = None;
        }
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

impl Slot for VirtualResource {
    fn id(&self) -> u64 {
        self.uid
    }

    fn set_id(&mut self, id: u64) {
        self.uid = id;
    }

    fn new(id: u64) -> Self {
        Self {
            uid: id,
            generation: 0,
            ref_count: None,
            type_id: TypeId::of::<()>(),
        }
    }
}

impl SlotWithGeneration for VirtualResource {
    fn generation(&self) -> u64 {
        self.generation
    }

    fn set_generation(&mut self, generation: u64) {
        self.generation = generation;
    }

    fn new_with_gen(id: u64, generation: u64) -> Self {
        Self {
            uid: id,
            generation,
            ref_count: None,
            type_id: TypeId::of::<()>(),
        }
    }
}

impl std::fmt::Debug for VirtualResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualResource")
            .field("uid", &self.uid)
            .field("generation", &self.generation)
            .field("type_id", &self.type_id)
            .field("has_ref_count", &self.ref_count.is_some())
            .finish()
    }
}

// Make VirtualResource Send since it's used across threads
unsafe impl Send for VirtualResource {}
