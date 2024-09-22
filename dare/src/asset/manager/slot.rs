use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::RwLock;
pub use super::super::prelude as asset;

// A slot for the asset container and contains the tll and t of the slot alongside the [`AssetHolder`]
#[derive(Debug)]
pub struct AssetContainerSlot<T: 'static + asset::AssetDescriptor> {
    pub(super) ttl: usize,
    pub(super) t: Arc<AtomicUsize>,
    pub(super) holder: asset::AssetHolder<T>,
}

impl<T: asset::AssetDescriptor> Clone for AssetContainerSlot<T> {
    fn clone(&self) -> Self {
        Self {
            ttl: self.ttl.clone(),
            t: self.t.clone(),
            holder: self.holder.clone()
        }
    }
}

impl<T: asset::AssetDescriptor> AssetContainerSlot<T> {
    pub fn get_holder(&self) -> &asset::AssetHolder<T> {
        self.t.store(self.ttl, Ordering::Release);
        &self.holder
    }
}

/// A reference to the underlying asset that only contains a hash
#[derive(Debug, Clone)]
pub struct AssetSlotRef<T: asset::AssetDescriptor> {
    pub(super) hash: u64,
    pub(super) holder: Arc<RwLock<asset::AssetState<T>>>,
    /// A reference to the deferred deletion
    pub(super) t_ref: Weak<AtomicUsize>,
    /// Pre-defined ttl
    pub(super) ttl: usize,
}

impl<T: asset::AssetDescriptor> PartialEq for AssetSlotRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl<T: asset::AssetDescriptor> Eq for AssetSlotRef<T> {}

impl<T: asset::AssetDescriptor> Hash for AssetSlotRef<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl<T: asset::AssetDescriptor> From<AssetContainerSlot<T>> for AssetSlotRef<T> {
    fn from(value: AssetContainerSlot<T>) -> Self {
        let mut hash = std::hash::DefaultHasher::new();
        value.holder.metadata.hash(&mut hash);
        let hash = hash.finish();
        Self {
            hash,
            holder: value.holder.state.clone(),
            t_ref: Arc::downgrade(&value.t),
            ttl: value.ttl,
        }
    }
}

impl<T: asset::AssetDescriptor> Deref for AssetSlotRef<T> {
    type Target = Arc<RwLock<asset::AssetState<T>>>;

    fn deref(&self) -> &Self::Target {
        if let Some(t) = self.t_ref.upgrade() {
            t.store(self.ttl, Ordering::Release);
        }
        &self.holder
    }
}

/// A weak reference to the underlying asset that only contains a hash
#[derive(Debug, Clone)]
pub struct WeakAssetSlotRef<T: asset::AssetDescriptor> {
    pub(super) hash: u64,
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
        let hash = hash.finish();
        Self {
            hash,
            holder: Arc::downgrade(&value.holder.state),
            t_ref: Arc::downgrade(&value.t),
            ttl: value.ttl,
        }
    }
}

impl<T: asset::AssetDescriptor> WeakAssetSlotRef<T> {
    /// Upgrade to a regular [`AssetSlotRef<T>`]
    pub fn upgrade(&self) -> Option<AssetSlotRef<T>> {
        Some(AssetSlotRef {
            hash: self.hash,
            holder: self.holder.upgrade()?,
            t_ref: self.t_ref.clone(),
            ttl: self.ttl,
        })
    }

    /// Tries to get a strong ref to the asset state
    pub fn get_strong(&self) -> Option<Arc<RwLock<asset::AssetState<T>>>> {
        Some(self.holder.upgrade()?)
    }
}