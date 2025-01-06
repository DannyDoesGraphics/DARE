use super::prelude as asset;
use derivative::Derivative;
use std::fmt::{Debug, Formatter, Pointer};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, Weak};

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct InternalHandle {
    pub(super) index: u32,
    pub(super) generation: u32,
}
impl From<asset::AssetIdUntyped> for InternalHandle {
    fn from(value: asset::AssetIdUntyped) -> Self {
        match value {
            asset::AssetIdUntyped::MetadataHash { id, type_id } => panic!(),
            asset::AssetIdUntyped::Generation { id, generation, .. } => Self {
                index: id,
                generation,
            },
        }
    }
}

pub enum AssetHandle<T: asset::Asset> {
    Strong(Arc<StrongAssetHandleUntyped>),
    Weak {
        weak_ref: Weak<StrongAssetHandleUntyped>,
        id: asset::AssetId<T>,
    },
}
impl<T: asset::Asset> PartialEq for AssetHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AssetHandle::Strong(a), AssetHandle::Strong(b)) => a.id == b.id,
            (AssetHandle::Weak { id, .. }, AssetHandle::Weak { id: id_b, .. }) => id == id_b,
            (AssetHandle::Strong(a), AssetHandle::Weak {id, ..}) => a.id == id.as_untyped_id(),
            (AssetHandle::Weak{id, ..}, AssetHandle::Strong(b)) => id.as_untyped_id() == b.id,
        }
    }
}
impl<T: asset::Asset> Eq for AssetHandle<T> {}

impl<T: asset::Asset> Hash for AssetHandle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            AssetHandle::Strong(arc) => arc.id.hash(state),
            AssetHandle::Weak { id, .. } => id.hash(state),
        }
    }
}
impl<T: asset::Asset> Debug for AssetHandle<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetHandle::Strong(arc) => Debug::fmt(arc, f),
            AssetHandle::Weak { id, .. } => Debug::fmt(id, f),
        }
    }
}
impl<T: asset::Asset> Clone for AssetHandle<T> {
    fn clone(&self) -> Self {
        match self {
            AssetHandle::Strong(arc) => AssetHandle::Strong(arc.clone()),
            AssetHandle::Weak { id, weak_ref } => AssetHandle::Weak {
                weak_ref: weak_ref.clone(),
                id: *id,
            },
        }
    }
}
impl<T: asset::Asset> AssetHandle<T> {
    /// Get the id
    pub fn id(&self) -> asset::AssetId<T> {
        match self {
            AssetHandle::Strong(arc) => arc.id.into_typed_id::<T>().unwrap(),
            AssetHandle::Weak { id, .. } => id.clone(),
        }
    }

    /// Convert to untyped handle
    pub fn into_untyped_handle(self) -> AssetHandleUntyped {
        match self {
            AssetHandle::Strong(arc) => AssetHandleUntyped::Strong(arc),
            AssetHandle::Weak { weak_ref, id } => AssetHandleUntyped::Weak {
                id: id.as_untyped_id(),
                weak_ref,
            },
        }
    }

    /// Downgrade
    pub fn downgrade(self) -> Self {
        self.into_untyped_handle()
            .downgrade()
            .into_typed_handle::<T>()
            .unwrap()
    }

    /// Upgrade
    pub fn upgrade(self) -> Option<Self> {
        Some(
            self.into_untyped_handle()
                .upgrade()?
                .into_typed_handle()
                .unwrap(),
        )
    }
}

/// Represents a wrapper struct for the id, but also a drop queue which will it will send it's
/// [`Self::id`], upon being dropped
#[derive(Derivative)]
#[derivative(PartialEq, Debug)]
pub(super) struct StrongAssetHandleUntyped {
    pub(super) id: asset::AssetIdUntyped,
    #[derivative(Debug = "ignore", PartialEq = "ignore")]
    pub(super) drop_send: crossbeam_channel::Sender<asset::AssetIdUntyped>,
}
impl Eq for StrongAssetHandleUntyped {}

impl Drop for StrongAssetHandleUntyped {
    fn drop(&mut self) {
        if let Err(_) = self.drop_send.send(self.id) {
            // do not care if the asset drop request was not received (asset manager dropped)
        }
    }
}
impl Hash for StrongAssetHandleUntyped {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// Untyped asset handles, keeps track of asset usage
#[derive(Debug)]
pub enum AssetHandleUntyped {
    Strong(Arc<StrongAssetHandleUntyped>),
    Weak {
        id: asset::AssetIdUntyped,
        weak_ref: Weak<StrongAssetHandleUntyped>,
    },
}
impl Hash for AssetHandleUntyped {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            AssetHandleUntyped::Strong(arc) => arc.id.hash(state),
            AssetHandleUntyped::Weak { id, .. } => id.hash(state),
        }
    }
}
impl Eq for AssetHandleUntyped {}
impl PartialEq for AssetHandleUntyped {
    fn eq(&self, other: &Self) -> bool {
        let s_id = match self {
            AssetHandleUntyped::Strong(arc) => &arc.id,
            AssetHandleUntyped::Weak { id, .. } => id,
        };
        let o_id = match other {
            AssetHandleUntyped::Strong(arc) => &arc.id,
            AssetHandleUntyped::Weak { id, .. } => id,
        };
        s_id == o_id
    }
}

impl Deref for AssetHandleUntyped {
    type Target = asset::AssetIdUntyped;

    fn deref(&self) -> &Self::Target {
        match self {
            AssetHandleUntyped::Strong(arc) => &arc.id,
            AssetHandleUntyped::Weak { id, .. } => id,
        }
    }
}
impl AssetHandleUntyped {
    pub fn into_typed_handle<T: asset::Asset>(self) -> Option<AssetHandle<T>> {
        match self {
            AssetHandleUntyped::Strong(arc) => Some(AssetHandle::Strong(arc)),
            AssetHandleUntyped::Weak { id, weak_ref } => Some(AssetHandle::Weak {
                id: id.into_typed_id()?,
                weak_ref,
            }),
        }
    }

    pub fn get_id(&self) -> asset::AssetIdUntyped {
        match self {
            AssetHandleUntyped::Strong(arc) => {
                arc.id.clone()
            }
            AssetHandleUntyped::Weak { id, .. } => {
                id.clone()
            }
        }
    }

    /// Tries to downgrade, if already weak, does nothing
    pub fn downgrade(self) -> Self {
        match self {
            AssetHandleUntyped::Strong(arc) => AssetHandleUntyped::Weak {
                id: arc.id.clone(),
                weak_ref: Arc::downgrade(&arc),
            },
            AssetHandleUntyped::Weak { .. } => self,
        }
    }

    /// Tries to upgrade, if weak, does nothing
    pub fn upgrade(self) -> Option<Self> {
        match self {
            AssetHandleUntyped::Strong(..) => Some(self),
            AssetHandleUntyped::Weak { weak_ref, .. } => {
                Some(AssetHandleUntyped::Strong(weak_ref.upgrade()?))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Define a simple test asset that meets the trait requirements.
    #[derive(Hash, Clone)]
    struct TestAssetMetadata(u64);
    impl asset::AssetMetadata for TestAssetMetadata {}

    impl Debug for TestAssetMetadata {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            todo!()
        }
    }

    impl PartialEq for TestAssetMetadata {
        fn eq(&self, other: &Self) -> bool {
            todo!()
        }
    }

    impl Eq for TestAssetMetadata {}

    impl asset::AssetLoaded for TestAssetMetadata {

    }

    impl asset::loaders::MetaDataLoad for TestAssetMetadata {
        type Loaded = TestAssetLoaded;
        type LoadInfo<'a>
        where
            Self: 'a
        = ();

        async fn load<'a>(&self, load_info: Self::LoadInfo<'a>) -> anyhow::Result<Self::Loaded> {
            todo!()
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    struct TestAssetLoaded(u64);
    impl asset::AssetLoaded for TestAssetLoaded {}

    struct TestAsset;
    impl asset::Asset for TestAsset {
        type Metadata = TestAssetMetadata;
        type Loaded = TestAssetLoaded;
    }

    fn hash_handle<H: Hash>(h: &H) -> u64 {
        let mut hasher = DefaultHasher::new();
        h.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn test_hashing_consistency() {
        let (tx, _rx) = crossbeam_channel::unbounded();

        let typed_id: asset::AssetId<TestAsset> = asset::AssetId::Generation { id: 42, generation: 1 };
        let untyped_id = typed_id.as_untyped_id();

        let strong_untyped = StrongAssetHandleUntyped {
            id: untyped_id.clone(),
            drop_send: tx,
        };

        let arc = Arc::new(strong_untyped);
        let strong_typed_handle = AssetHandle::<TestAsset>::Strong(arc.clone());

        let initial_hash = hash_handle(&strong_typed_handle);

        let downgraded = strong_typed_handle.clone().downgrade();
        let downgraded_hash = hash_handle(&downgraded);

        let upgraded = downgraded.clone().upgrade().expect("Should be able to upgrade");
        let upgraded_hash = hash_handle(&upgraded);

        assert_eq!(initial_hash, downgraded_hash, "Hash should remain the same after downgrade");
        assert_eq!(initial_hash, upgraded_hash, "Hash should remain the same after upgrade");
    }
}
