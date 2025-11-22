use super::prelude as asset;
use std::any::TypeId;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
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
impl Hash for AssetIdUntyped {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            AssetIdUntyped::MetadataHash { id, type_id } => {
                0.hash(state);
                id.hash(state);
                type_id.hash(state);
            }
            AssetIdUntyped::Generation {
                id,
                generation,
                type_id,
            } => {
                1.hash(state);
                id.hash(state);
                generation.hash(state);
                type_id.hash(state);
            }
        }
    }
}
impl PartialOrd<Self> for AssetIdUntyped {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (AssetIdUntyped::MetadataHash { .. }, AssetIdUntyped::Generation { .. }) => {
                Some(Ordering::Less)
            }
            (AssetIdUntyped::Generation { .. }, AssetIdUntyped::MetadataHash { .. }) => {
                Some(Ordering::Greater)
            }
            (
                AssetIdUntyped::MetadataHash { id, .. },
                AssetIdUntyped::MetadataHash { id: id_b, .. },
            ) => id.partial_cmp(id_b),
            (
                AssetIdUntyped::Generation { generation, id, .. },
                AssetIdUntyped::Generation {
                    generation: generation_b,
                    id: id_b,
                    ..
                },
            ) => Some(match (id.cmp(id_b), generation.cmp(generation_b)) {
                (Ordering::Equal, Ordering::Equal) => Ordering::Equal,
                (Ordering::Less, _) => Ordering::Less,
                (Ordering::Greater, _) => Ordering::Greater,
                (Ordering::Equal, Ordering::Less) => Ordering::Less,
                (Ordering::Equal, Ordering::Greater) => Ordering::Greater,
            }),
        }
    }
}

impl Ord for AssetIdUntyped {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
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
            AssetId::Generation {
                generation,
                id: index,
            } => AssetIdUntyped::Generation {
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
                AssetIdUntyped::Generation { id, generation, .. } => {
                    Some(AssetId::Generation { id, generation })
                }
            }
        } else {
            None
        }
    }
}
pub enum AssetId<T: super::traits::Asset> {
    MetadataHash(u64),
    Generation { id: u32, generation: u32 },
    Phantom(PhantomData<T>),
}
unsafe impl<T: super::traits::Asset> Send for AssetId<T> {}
unsafe impl<T: super::traits::Asset> Sync for AssetId<T> {}
impl<T: super::traits::Asset> Debug for AssetId<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            AssetId::MetadataHash(hash) => {
                write!(f, "AssetId::MetadataHash({})", hash)
            }
            AssetId::Generation {
                id: index,
                generation,
            } => {
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
                    id: a_index,
                    generation: a_generation,
                },
                &AssetId::Generation {
                    id: b_index,
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
        match self {
            AssetId::MetadataHash(id) => {
                0.hash(state);
                id.hash(state);
                TypeId::of::<T>().hash(state);
            }
            AssetId::Generation {
                id: index,
                generation,
                ..
            } => {
                1.hash(state);
                index.hash(state);
                generation.hash(state);
                TypeId::of::<T>().hash(state);
            }
            AssetId::Phantom(_) => panic!("Phantom type cannot be hashed"),
        }
    }
}

impl<T: super::traits::Asset> Clone for AssetId<T> {
    fn clone(&self) -> Self {
        match self {
            AssetId::MetadataHash(u64) => AssetId::MetadataHash(*u64),
            AssetId::Generation {
                id: index,
                generation,
                ..
            } => AssetId::Generation {
                id: *index,
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
            AssetIdUntyped::Generation { id, generation, .. } => {
                AssetId::Generation { id, generation }
            }
        }
    }
}

impl<T: super::traits::Asset> AssetId<T> {
    pub fn as_untyped_id(self) -> AssetIdUntyped {
        match self {
            AssetId::MetadataHash(hash) => AssetIdUntyped::MetadataHash {
                id: hash,
                type_id: TypeId::of::<T>(),
            },
            AssetId::Generation { id, generation } => AssetIdUntyped::Generation {
                id,
                generation,
                type_id: TypeId::of::<T>(),
            },
            AssetId::Phantom(_) => panic!(),
        }
    }
}
