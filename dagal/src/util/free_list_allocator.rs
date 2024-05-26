use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use crate::traits::Destructible;
use anyhow::Result;

#[derive(Copy, PartialOrd, PartialEq, Eq, Debug, Hash)]
pub struct Handle<T> {
	id: u64,
	_marker: PhantomData<T>,
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


/// Free list allocator
///
/// # Differences from [`SlotMap`](crate::util::SlotMap)
/// Unlike a slot map, a free list allocator will ensure that there is a direct 1:1 mapping always
/// from the handle's id to the resource's index.
///
/// Slot maps will frequently swap resources around to ensure more efficient iteration of memory, but
/// in cases where we value coherent 1:1 resource id to resource index mappings, free list allocators
/// make more sense.
#[derive(Debug)]
struct FreeListInner<T> {
	resources: Vec<Option<T>>,
	free_ids: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct FreeList<T> {
	inner: Arc<RwLock<FreeListInner<T>>>,
}

impl<T: Clone> Default for FreeList<T> {
	fn default() -> Self {
		Self {
			inner: Arc::new(RwLock::new(FreeListInner {
				resources: vec![],
				free_ids: vec![],
			}))
		}
	}
}

impl<T: Clone> FreeList<T> {
	pub fn allocate(&mut self, resource: T) -> Result<Handle<T>> {
		let mut guard = self.inner.write().map_err(|err| {
			anyhow::Error::from(crate::DagalError::PoisonError)
		})?;
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
		let mut guard = self.inner.write().map_err(|err| {
			anyhow::Error::from(crate::DagalError::PoisonError)
		})?;
		let resource: Option<T> = guard.resources.get_mut(handle.id as usize).and_then(Option::take);
		guard.free_ids.push(handle.id);
		Ok(resource.unwrap())
	}

	pub fn get(&self, handle: &Handle<T>) -> Result<T> {
		unsafe {
			self.untyped_get(handle)
		}
	}

	pub fn is_valid(&self, handle: &Handle<T>) -> Result<bool> {
		unsafe {
			self.untyped_is_valid(handle)
		}
	}

	pub(crate) unsafe fn untyped_is_valid<A>(&self, handle: &Handle<A>) -> Result<bool> {
		if let Some(resource) = self.inner.read().map_err(|err| {
			anyhow::Error::from(crate::DagalError::PoisonError)
		})?.resources.get(handle.id as usize) {
			return Ok(resource.is_some());
		}
		Ok(false)
	}

	/// Get the underlying values regardless of handle type
	pub(crate) unsafe fn untyped_get<A>(&self, handle: &Handle<A>) -> Result<T> {
		if unsafe { !self.untyped_is_valid(handle)? } {
			return Err(anyhow::Error::from(errors::Errors::InvalidHandle));
		}
		let guard = self.inner.read().map_err(|err| {
			anyhow::Error::from(crate::DagalError::PoisonError)
		})?;
		let resource = guard.resources.get(handle.id as usize);
		let resource = resource.unwrap().as_ref().cloned().unwrap();

		Ok(resource)
	}

	pub fn with_iter_mut<F: FnOnce(&mut dyn Iterator<Item = &mut Option<T>>)>(&mut self, f: F) {
		let mut guard = self.inner.write().map_err(|_| {
			anyhow::Error::from(crate::DagalError::PoisonError)
		}).unwrap();

		let mut iter = guard.resources.iter_mut();
		f(&mut iter);
	}
}

impl<T: Clone + Destructible> FreeList<T> {

	/// Performs a de-allocation but also destroys the resource
	pub fn deallocate_destructible(&mut self, handle: Handle<T>) -> Result<()> {
		let mut resource = self.deallocate(handle)?;
		resource.destroy();
		Ok(())
	}
}

pub mod errors {
	use thiserror::Error;

	#[derive(Debug, Error)]
	pub enum Errors {
		#[error("Handle does not exist in the free list allocator")]
		InvalidHandle
	}
}