use anyhow::Result;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Pointer};
use std::sync::{Arc, Mutex, MutexGuard};

/// Flash map implementation of erased storage

/// A flash map backed type erased storage
///
/// # Performance characteristics
/// [`FlashMapErasedStorage`] is should only be used in work loads with **read-heavy to read-only**
/// work loads. It should never be used when there are many writes involved.
///
/// ## One writer
/// Only one writer can exist meaning writing from multiple threads must be done using channels or
/// awaiting on the internal mutex
///
/// ## Async
/// The entire struct is async compatible
#[derive(Clone)]
pub struct FlashMapErasedStorage {
    read_handle: Arc<flashmap::ReadHandle<TypeId, Box<dyn Any>>>,
    write_handle: Arc<Mutex<flashmap::WriteHandle<TypeId, Box<dyn Any>>>>,
}
unsafe impl Send for FlashMapErasedStorage {}
unsafe impl Sync for FlashMapErasedStorage {}
impl Debug for FlashMapErasedStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "")
    }
}

impl FlashMapErasedStorage {
    pub fn new() -> Self {
        let (write, read) = flashmap::new::<TypeId, Box<dyn Any>>();
        Self {
            write_handle: Arc::new(Mutex::new(write)),
            read_handle: Arc::new(read),
        }
    }

    /// Get a mutable write mutex
    pub fn get_mut_write_guard(
        &self,
    ) -> Result<MutexGuard<flashmap::WriteHandle<TypeId, Box<dyn Any>>>> {
        Ok(self
            .write_handle
            .lock()
            .map_err(|_| anyhow::Error::from(anyhow::anyhow!("Poison error")))?)
    }

    /// Get the write handle
    pub fn get_write(&self) -> &Arc<Mutex<flashmap::WriteHandle<TypeId, Box<dyn Any>>>> {
        &self.write_handle
    }

    pub fn with<T: 'static, R, F: FnOnce(&T) -> R>(&self, f: F) -> Option<R> {
        self.read_handle
            .guard()
            .get(&TypeId::of::<T>())
            .and_then(|data| data.downcast_ref::<T>().map(f))
    }
}

impl From<HashMap<TypeId, Box<dyn Any>>> for FlashMapErasedStorage {
    fn from(value: HashMap<TypeId, Box<dyn Any>>) -> Self {
        let mut flashmap = Self::new();
        for (key, value) in value.into_iter() {
            let mut guard = flashmap.write_handle.lock().unwrap();
            guard.guard().insert(key, value).unwrap();
        }
        flashmap
    }
}
