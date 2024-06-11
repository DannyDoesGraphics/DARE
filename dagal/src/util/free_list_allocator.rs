use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use derivative::Derivative;

use crate::resource::traits::Resource;
use crate::traits::Destructible;

#[derive(Copy, PartialOrd, Eq)]
pub struct Handle<T> {
    id: u64,
    _marker: PhantomData<T>,
}

impl<T> Debug for Handle<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle").field("id", &self.id).finish()
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _marker: Default::default(),
        }
    }
}

impl<T> Handle<T> {
    pub fn id(&self) -> u64 {
        self.id
    }

    /// If for whatever reason we wish to manually set the id ourselves.
    pub(crate) unsafe fn set_id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }

    pub fn new(id: u64) -> Self {
        Self {
            id,
            _marker: Default::default(),
        }
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// Free list allocator
///
/// # Differences from [`SlotMap`](crate::util::DenseSlotMap)
/// Unlike a slot map, a free list allocator will ensure that there is a direct 1:1 mapping always
/// from the handle's id to the resource's index.
///
/// Slot maps will frequently swap resources around to ensure more efficient iteration of memory, but
/// in cases where we value coherent 1:1 resource id to resource index mappings, free list allocators
/// make more sense.
#[derive(Derivative)]
#[derivative(Debug)]
struct FreeListInner<T> {
    #[derivative(Debug = "ignore")]
    resources: Vec<Option<T>>,
    free_ids: Vec<u64>,
}

#[derive(Debug)]
pub struct FreeList<T> {
    inner: Arc<RwLock<FreeListInner<T>>>,
}

impl<T> Clone for FreeList<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Default for FreeList<T> {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(FreeListInner {
                resources: vec![],
                free_ids: vec![],
            })),
        }
    }
}

impl<T> FreeList<T> {
    pub fn allocate(&mut self, resource: T) -> Result<Handle<T>> {
        let mut guard = self
            .inner
            .write()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        let id: u64 = if guard.free_ids.is_empty() {
            guard.resources.len() as u64
        } else {
            guard.free_ids.remove(0)
        };

        guard.resources.push(Some(resource));
        Ok(Handle {
            id,
            _marker: Default::default(),
        })
    }

    pub fn deallocate(&mut self, handle: Handle<T>) -> Result<T> {
        if !self.is_valid(&handle)? {
            return Err(anyhow::Error::from(errors::Errors::InvalidHandle));
        }
        let mut guard = self
            .inner
            .write()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?;
        let resource: Option<T> = guard
            .resources
            .get_mut(handle.id as usize)
            .and_then(Option::take);
        guard.free_ids.push(handle.id);
        Ok(resource.unwrap())
    }

    pub fn with_handle<R, F: FnOnce(&T) -> R>(&self, handle: &Handle<T>, f: F) -> Result<R> {
        unsafe { self.untyped_with_handle(handle, f) }
    }

    pub fn with_handle_mut<R, F: FnOnce(&mut T) -> R>(
        &self,
        handle: &Handle<T>,
        f: F,
    ) -> Result<R> {
        unsafe { self.untyped_with_handle_mut(handle, f) }
    }

    pub fn is_valid(&self, handle: &Handle<T>) -> Result<bool> {
        unsafe { self.untyped_is_valid(handle) }
    }

    pub(crate) unsafe fn untyped_is_valid<A>(&self, handle: &Handle<A>) -> Result<bool> {
        if let Some(resource) = self
            .inner
            .read()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?
            .resources
            .get(handle.id as usize)
        {
            return Ok(resource.is_some());
        }
        Ok(false)
    }

    /// Execute with a handle's underlying resource
    ///
    /// Result returned if the lambda's return type. Any error in the Result indicates an error
    /// found with the handle.
    pub unsafe fn untyped_with_handle<A, R, F: FnOnce(&T) -> R>(
        &self,
        handle: &Handle<A>,
        f: F,
    ) -> Result<R> {
        if !self.untyped_is_valid(handle)? {
            return Err(anyhow::Error::from(errors::Errors::InvalidHandle));
        }
        self.inner
            .read()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?
            .resources
            .get(handle.id as usize)
            .unwrap()
            .as_ref()
            .map_or(
                Err(anyhow::Error::from(crate::DagalError::PoisonError)),
                |data| Ok(f(data)),
            )
    }

    /// Execute with a handle's underlying resource
    ///
    /// Result returned if the lambda's return type. Any error in the Result indicates an error
    /// found with the handle.
    pub unsafe fn untyped_with_handle_mut<A, R, F: FnOnce(&mut T) -> R>(
        &self,
        handle: &Handle<A>,
        f: F,
    ) -> Result<R> {
        if !self.untyped_is_valid(handle)? {
            return Err(anyhow::Error::from(errors::Errors::InvalidHandle));
        }
        self.inner
            .write()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?
            .resources
            .get_mut(handle.id as usize)
            .unwrap()
            .as_mut()
            .map_or(
                Err(anyhow::Error::from(crate::DagalError::PoisonError)),
                |data| Ok(f(data)),
            )
    }
}

impl<'a, T: Resource<'a>> FreeList<T> {
    /// If you're simply acquiring a resource's handle
    pub fn get_resource_handle(&self, handle: &Handle<T>) -> Result<T::HandleType> {
        if !self.is_valid(handle)? {
            return Err(anyhow::Error::from(errors::Errors::InvalidHandle));
        }
        Ok(self
            .inner
            .read()
            .map_err(|_| anyhow::Error::from(crate::DagalError::PoisonError))?
            .resources
            .get(handle.id as usize)
            .unwrap()
            .as_ref()
            .unwrap()
            .handle())
    }
}

impl<T: Destructible> FreeList<T> {
    /// Deallocates the handle's resource as well calls destroy automatically
    pub fn deallocate_destructible(&mut self, handle: Handle<T>) -> Result<()> {
        let res = self.deallocate(handle)?;
        drop(res);
        Ok(())
    }
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum Errors {
        #[error("Handle does not exist in the free list allocator")]
        InvalidHandle,
    }
}
