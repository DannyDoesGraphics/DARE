pub use super::super::prelude as asset;
use crate::asset::manager::MetadataHash;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use tokio::sync::{MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// The holds the [`asset::AssetMetadataAndState<T>`] alongside the asset's lifetime, `t`, and it's pre-determined
/// `ttl` (time to live)
#[derive(Debug)]
pub struct AssetContainerSlot<T: 'static + asset::AssetDescriptor> {
    pub(super) ttl: usize,
    pub(super) t: Arc<AtomicUsize>,
    pub(super) holder: asset::AssetMetadataAndState<T>,
}

impl<T: asset::AssetDescriptor> Clone for AssetContainerSlot<T> {
    fn clone(&self) -> Self {
        Self {
            ttl: self.ttl.clone(),
            t: self.t.clone(),
            holder: self.holder.clone(),
        }
    }
}

impl<T: asset::AssetDescriptor> AssetContainerSlot<T> {
    pub fn get_holder(&self) -> &asset::AssetMetadataAndState<T> {
        self.t.store(self.ttl, Ordering::Release);
        &self.holder
    }

    /// Keep the asset holder alive
    pub fn keep_alive(&self) {
        self.t.fetch_add(self.ttl, Ordering::Release);
    }
}

/// A reference to the underlying asset that only contains a hash
///
/// # Difference from [`AssetContainerSlot`]
/// Unlike AssetContainerSlot, we only store the 64-bit hash of the [`T::Metadata`].
///
/// tl;dr we only store the index into the [`super::AssetManager`]
///
/// # Deref
/// Dereferencing to access the underlying asset state, will result in the time to live to increase by
/// the stored ttl
#[derive(Debug, Clone)]
pub struct AssetSlotRef<T: asset::AssetDescriptor> {
    pub(super) hash: MetadataHash,
    pub(super) holder: Arc<RwLock<asset::AssetState<T>>>,
    /// A reference to the asset's lifetime
    pub(super) t_ref: Arc<AtomicUsize>,
    /// Pre-defined ttl
    pub(super) ttl: usize,
}
unsafe impl<T: asset::AssetDescriptor> Send for AssetSlotRef<T> {}
unsafe impl<T: asset::AssetDescriptor> Sync for AssetSlotRef<T> {}

impl<T: asset::AssetDescriptor> PartialEq for AssetSlotRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl<T: asset::AssetDescriptor> Eq for AssetSlotRef<T> {}
impl<T: asset::AssetDescriptor> Hash for AssetSlotRef<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl<T: asset::AssetDescriptor> From<&AssetContainerSlot<T>> for AssetSlotRef<T> {
    fn from(value: &AssetContainerSlot<T>) -> Self {
        let mut hash = std::hash::DefaultHasher::new();
        value.holder.metadata.hash(&mut hash);
        let hash: MetadataHash = hash.finish().into();
        Self {
            hash,
            holder: value.holder.state.clone(),
            t_ref: value.t.clone(),
            ttl: value.ttl,
        }
    }
}

impl<T: asset::AssetDescriptor> Deref for AssetSlotRef<T> {
    type Target = Arc<RwLock<asset::AssetState<T>>>;

    fn deref(&self) -> &Self::Target {
        self.t_ref.store(self.ttl, Ordering::Release);
        &self.holder
    }
}

impl<T: asset::AssetDescriptor> AssetSlotRef<T> {
    /// Keep the asset reference alive
    pub fn keep_alive(&self) {
        self.t_ref.store(self.ttl, Ordering::Release);
    }
}

/// A weak reference to the underlying asset that only contains a hash
///
/// Contains a weak reference to the underlying [`asset::AssetState`] as well the time atomic
/// which determines the lifetime of the asset
///
/// # Safety
/// There is no guarantee using
#[derive(Debug, Clone)]
pub struct WeakAssetSlotRef<T: asset::AssetDescriptor> {
    pub(super) hash: super::MetadataHash,
    pub(super) holder: Weak<RwLock<asset::AssetState<T>>>,
    /// A reference to the deferred deletion
    pub(super) t_ref: Weak<AtomicUsize>,
    /// Pre-defined ttl
    pub(super) ttl: usize,
}

impl<T: asset::AssetDescriptor> From<AssetContainerSlot<T>> for WeakAssetSlotRef<T> {
    fn from(value: AssetContainerSlot<T>) -> Self {
        let mut hash = std::hash::DefaultHasher::new();
        value.holder.metadata.hash(&mut hash);
        let hash: MetadataHash = hash.finish().into();
        Self {
            hash,
            holder: Arc::downgrade(&value.holder.state),
            t_ref: Arc::downgrade(&value.t),
            ttl: value.ttl,
        }
    }
}

// todo: we need to consider the case where we're unable to properly dereference the weak ptr
impl<T: asset::AssetDescriptor> WeakAssetSlotRef<T> {
    /// Upgrade to a regular [`AssetSlotRef<T>`]
    pub fn upgrade(&self) -> Option<AssetSlotRef<T>> {
        Some(AssetSlotRef {
            hash: self.hash,
            holder: self.holder.upgrade()?,
            t_ref: self.t_ref.upgrade()?,
            ttl: self.ttl,
        })
    }

    /// Tries to get a strong ref to the asset state
    pub fn get_strong(&self) -> Option<Arc<RwLock<asset::AssetState<T>>>> {
        Some(self.holder.upgrade()?)
    }
}

#[derive(Debug)]
pub struct AssetSlotReadGuard<'a, T: asset::AssetDescriptor> {
    guard: RwLockReadGuard<'a, asset::AssetState<T>>,
    t: Arc<AtomicUsize>,
}
unsafe impl<'a, T: asset::AssetDescriptor> Send for AssetSlotReadGuard<'a, T> {}
impl<'a, T: asset::AssetDescriptor> Deref for AssetSlotReadGuard<'a, T> {
    type Target = asset::AssetState<T>;

    fn deref(&self) -> &Self::Target {
        &*self.guard
    }
}
impl<'a, T: asset::AssetDescriptor> Into<RwLockReadGuard<'a, asset::AssetState<T>>>
    for AssetSlotReadGuard<'a, T>
{
    fn into(self) -> RwLockReadGuard<'a, asset::AssetState<T>> {
        self.guard
    }
}

pub struct AssetSlotWriteGuard<'a, T: asset::AssetDescriptor> {
    guard: RwLockWriteGuard<'a, asset::AssetState<T>>,
    t: Arc<AtomicUsize>,
}
unsafe impl<'a, T: asset::AssetDescriptor> Send for AssetSlotWriteGuard<'a, T> {}
impl<'a, T: asset::AssetDescriptor> Deref for AssetSlotWriteGuard<'a, T> {
    type Target = asset::AssetState<T>;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}
impl<'a, T: asset::AssetDescriptor> DerefMut for AssetSlotWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}
impl<'a, T: asset::AssetDescriptor> Into<RwLockWriteGuard<'a, asset::AssetState<T>>>
    for AssetSlotWriteGuard<'a, T>
{
    fn into(self) -> RwLockWriteGuard<'a, asset::AssetState<T>> {
        self.guard
    }
}
