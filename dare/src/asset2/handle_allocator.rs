use super::prelude as asset;
use std::sync::atomic::AtomicU32;
use std::sync::{atomic, Arc};

#[derive(Debug)]
pub struct HandleAllocator {
    pub next_index: AtomicU32,
    pub send_handle: crossbeam_channel::Sender<asset::InternalHandle>,
    pub recv_handle: crossbeam_channel::Receiver<asset::InternalHandle>,
}

impl Default for HandleAllocator {
    fn default() -> Self {
        let (send_handle, recv_handle) = crossbeam_channel::unbounded();
        Self {
            next_index: Default::default(),
            send_handle,
            recv_handle,
        }
    }
}
impl HandleAllocator {
    /// Get the next available handle
    pub fn get_next_handle(&self) -> asset::InternalHandle {
        self.recv_handle.try_recv().map_or(
            {
                let index: u32 = self.next_index.fetch_add(1, atomic::Ordering::Relaxed);
                println!("Done?");
                asset::InternalHandle {
                    index,
                    generation: 0,
                }
            },
            |mut handle| {
                handle.generation += 1;
                handle
            },
        )
    }

    /// Send a handle back
    pub fn recycle(&self, handle: asset::InternalHandle) {
        self.send_handle.send(handle).unwrap();
    }
}
