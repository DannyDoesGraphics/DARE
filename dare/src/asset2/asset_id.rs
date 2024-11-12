use super::prelude as asset;
use std::any::TypeId;
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;

#[derive(PartialEq, Eq, Copy, Clone, Hash, Debug)]
pub enum AssetIdUntyped {
    MetadataHash {
        id: u64,
        type_id: TypeId,
    },
    Generation {
        id: u32,
        generation: u32,
        type_id: TypeId,
    },
}

impl AssetIdUntyped {
    pub fn is_type<T: asset::Asset>(&self) -> bool {
        match self {
            AssetIdUntyped::MetadataHash { type_id, .. } => *type_id == TypeId::of::<T>(),
            AssetIdUntyped::Generation { type_id, .. } => *type_id == TypeId::of::<T>(),
        }
    }

    pub fn from_typed_handle<T: asset::Asset>(handle: asset::AssetHandle<T>) -> AssetIdUntyped {
        match handle {
            asset::AssetHandle::Strong(arc) => arc.id,
            asset::AssetHandle::Weak { id, .. } => AssetIdUntyped::from_type_asset_id(id),
        }
    }

    pub fn from_type_asset_id<T: asset::Asset>(id: AssetId<T>) -> Self {
        match id {
            AssetId::MetadataHash(id) => AssetIdUntyped::MetadataHash {
                id,
                type_id: TypeId::of::<T>(),
            },
            AssetId::Generation { generation, index } => AssetIdUntyped::Generation {
                id: index,
                generation,
                type_id: TypeId::of::<T>(),
            },
            AssetId::Phantom(_) => panic!(),
        }
    }

    pub fn into_typed_id<T: asset::Asset>(self) -> Option<AssetId<T>> {
        if self.is_type::<T>() {
            match self {
                AssetIdUntyped::MetadataHash { id, .. } => Some(AssetId::MetadataHash(id)),
                AssetIdUntyped::Generation { id, generation, .. } => Some(AssetId::Generation {
                    index: id,
                    generation,
                }),
            }
        } else {
            None
        }
    }
}
pub enum AssetId<T: super::traits::Asset> {
    MetadataHash(u64),
    Generation { index: u32, generation: u32 },
    Phantom(PhantomData<T>),
}
impl<T: super::traits::Asset> Debug for AssetId<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            AssetId::MetadataHash(hash) => {
                write!(f, "AssetId::MetadataHash({})", hash)
            }
            AssetId::Generation { index, generation } => {
                write!(
                    f,
                    "AssetId::Generation {{ index: {}, generation: {} }}",
                    index, generation
                )
            }
            AssetId::Phantom(_) => {
                write!(f, "AssetId::Phantom(PhantomData)")
            }
        }
    }
}
impl<T: super::traits::Asset> PartialEq for AssetId<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (&AssetId::MetadataHash(a), &AssetId::MetadataHash(b)) => a == b,
            (
                &AssetId::Generation {
                    index: a_index,
                    generation: a_generation,
                },
                &AssetId::Generation {
                    index: b_index,
                    generation: b_generation,
                },
            ) => a_index == b_index && a_generation == b_generation,
            (&AssetId::Phantom(_), _) => false,
            (_, &AssetId::Phantom(_)) => false,
            (&AssetId::MetadataHash(_), &AssetId::Generation { .. }) => false,
            (&AssetId::Generation { .. }, &AssetId::MetadataHash(_)) => false,
        }
    }
}

impl<T: super::traits::Asset> Eq for AssetId<T> {}
impl<T: super::traits::Asset> Hash for AssetId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let untyped = AssetIdUntyped::from_type_asset_id(self.clone());
        untyped.hash(state);
    }
}

impl<T: super::traits::Asset> Clone for AssetId<T> {
    fn clone(&self) -> Self {
        match self {
            AssetId::MetadataHash(u64) => AssetId::MetadataHash(*u64),
            AssetId::Generation {
                index, generation, ..
            } => AssetId::Generation {
                index: *index,
                generation: *generation,
            },
            AssetId::Phantom(_) => AssetId::Phantom(PhantomData),
        }
    }
}
impl<T: super::traits::Asset> Copy for AssetId<T> {}
impl<T: asset::Asset> From<AssetIdUntyped> for AssetId<T> {
    fn from(value: AssetIdUntyped) -> Self {
        match value {
            AssetIdUntyped::MetadataHash { id, .. } => AssetId::MetadataHash(id),
            AssetIdUntyped::Generation { id, generation, .. } => AssetId::Generation {
                index: id,
                generation,
            },
        }
    }
}

unsafe impl<T: super::traits::Asset> Send for AssetId<T> {}

impl<T: super::traits::Asset> AssetId<T> {
    pub fn as_untyped_id(self) -> AssetIdUntyped {
        match self {
            AssetId::MetadataHash(hash) => AssetIdUntyped::MetadataHash {
                id: hash,
                type_id: TypeId::of::<T>(),
            },
            AssetId::Generation { index, generation } => AssetIdUntyped::Generation {
                id: index,
                generation,
                type_id: TypeId::of::<T>(),
            },
            AssetId::Phantom(_) => panic!(),
        }
    }
}
