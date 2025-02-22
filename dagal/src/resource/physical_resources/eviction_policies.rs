use crate::resource::virtual_resources::VirtualResource;
use std::any::Any;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use tracing::Instrument;
use crate::resource::physical_resources::EvictionPolicy;

pub struct NoEvictionPolicy {}
impl NoEvictionPolicy {
    pub fn new() -> Self {
        Self {}
    }
}
impl EvictionPolicy for NoEvictionPolicy {
    type ResourceHandle = VirtualResource;

    fn on_insert(&mut self, key: VirtualResource) -> VirtualResource {
        key
    }

    fn on_access(&mut self, key: &VirtualResource) {
        todo!()
    }

    fn evict(&mut self, storage: &mut HashMap<VirtualResource, Box<dyn Any>>) {
        todo!()
    }
}

/// LRU eviction policy or (least recently used) defines a specific capacity then evicts if
/// we exceed such a capacity
#[derive(Debug)]
pub struct LruEvictionPolicy {
    /// Order keys have been accessed
    access_order: Vec<VirtualResource>,
    /// Capacity the storage can hold at most
    capacity: usize,
}
impl LruEvictionPolicy {
    pub fn new(capacity: usize) -> Self {
        Self {
            access_order: Vec::new(),
            capacity,
        }
    }
}
impl EvictionPolicy for LruEvictionPolicy {
    type ResourceHandle = VirtualResource;

    fn on_insert(&mut self, key: VirtualResource) -> VirtualResource {
        self.access_order.push(key.clone());
        key
    }

    fn on_access(&mut self, key: &VirtualResource) {
        self.access_order.push(key.clone());
    }

    fn evict(&mut self, storage: &mut HashMap<VirtualResource, Box<dyn Any>>) {
        if storage.len() > self.capacity {
            // grab the next n over capacity and drain them to be removed
            for key in self.access_order.drain(self.capacity..storage.len()) {
                storage.remove(&key);
            }
        }
    }
}

/// Indicates an instance of a virtual resource that when dropped, will immediately remove the underlying
/// resource key it holds
#[derive(Debug)]
pub struct VirtualResourceDrop {
    resource: VirtualResource,
    send_drop: crossbeam_channel::Sender<VirtualResource>,
}

impl Deref for VirtualResourceDrop {
    type Target = VirtualResource;

    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}
impl PartialEq for VirtualResourceDrop {
    fn eq(&self, other: &Self) -> bool {
        self.resource == other.resource
    }
}
impl PartialEq<VirtualResource> for VirtualResourceDrop {
    fn eq(&self, other: &VirtualResource) -> bool {
        *self == *other
    }
}
impl Drop for VirtualResourceDrop {
    fn drop(&mut self) {
        self.send_drop.send(self.resource.clone()).unwrap();
    }
}

/// Use a [`ArcEvictionPolicy`] to use reference counting to determine when to kick out
#[derive(Debug)]
pub struct ArcEvictionPolicy {
    drop_queue: crossbeam_channel::Receiver<VirtualResource>,
    send_queue: crossbeam_channel::Sender<VirtualResource>,
}

impl ArcEvictionPolicy {
    pub fn new() -> Self {
        let (send, recv) = crossbeam_channel::unbounded();
        Self {
            drop_queue: recv,
            send_queue: send,
        }
    }
}
impl EvictionPolicy for ArcEvictionPolicy {
    type ResourceHandle = Arc<VirtualResourceDrop>;

    fn on_insert(&mut self, key: VirtualResource) -> Arc<VirtualResourceDrop> {
        Arc::new(VirtualResourceDrop {
            resource: key,
            send_drop: self.send_queue.clone(),
        })
    }

    fn on_access(&mut self, key: &VirtualResource) {}

    fn evict(&mut self, storage: &mut HashMap<VirtualResource, Box<dyn Any>>) {
        // Go through drop queue and evict
        while let Ok(drop) = self.drop_queue.recv() {
            storage.remove(&drop);
        }
    }
}

/// Deploys a deletion queue to handle eviction
pub struct DeletionQueueEvictionPolicy {
    arc_eviction_policy: ArcEvictionPolicy,
    handle_map: HashMap<VirtualResource, (Arc<VirtualResourceDrop>, usize)>,
    // if a handle is accessed, it may still exist and as such we can still recover it
    handle_map_weak: HashMap<VirtualResource, Weak<VirtualResourceDrop>>,
    lifetime: usize,
}
impl DeletionQueueEvictionPolicy {
    pub fn new(lifetime: usize) -> Self {
        Self {
            arc_eviction_policy: ArcEvictionPolicy::new(),
            handle_map: Default::default(),
            handle_map_weak: Default::default(),
            lifetime,
        }
    }
}
impl EvictionPolicy for DeletionQueueEvictionPolicy {
    type ResourceHandle = Arc<VirtualResourceDrop>;

    fn on_insert(&mut self, key: VirtualResource) -> Self::ResourceHandle {
        let ar = self.arc_eviction_policy.on_insert(key.clone());
        self.handle_map.insert(key, (ar.clone(), self.lifetime));
        ar
    }

    fn on_access(&mut self, key: &VirtualResource) {
        match self.handle_map.get_mut(key) {
            None => {
                match self.handle_map_weak.remove(key).map(|handle| handle.upgrade()).flatten() {
                    None => {
                        // cannot access
                    }
                    Some(drop) => {
                        // recover back
                        self.handle_map.insert(*key, (
                            drop, self.lifetime
                        ));
                    }
                }
            }
            Some((_, lifetime)) => {
                *lifetime = self.lifetime;
            }
        }
    }

    fn evict(&mut self, storage: &mut HashMap<VirtualResource, Box<dyn Any>>) {
        // remove from internal map in hopes that we reduce rc -> 0 to induce a removal
        self.handle_map.retain(|_, (handle, lifetime)| {
            *lifetime = lifetime.saturating_sub(1);
            let vr: VirtualResource = handle.resource;
            if *lifetime == 0 {
                // if lifetime is zero, downgrade drop handle
                self.handle_map_weak.insert(vr, Arc::downgrade(handle));
                true
            } else {
                false
            }
        });
        self.arc_eviction_policy.evict(storage);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::resource::test::TestResource;
    use crate::resource::traits::Resource;
    use crate::resource::physical_resources::PhysicalResourceBindings;

    #[test]
    pub fn no_eviction_policy() {
        let mut map = PhysicalResourceBindings::new(NoEvictionPolicy::new());

        let r = TestResource::new(1).unwrap();
        let handle = map.insert(r.clone()).unwrap();
        assert!(map.insert(r.clone()).is_none());
    }
}
