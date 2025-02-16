use crate::prelude::asset2::AssetHandle;
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use dare_containers::prelude::Slot;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

/// Change in reference count
pub(super) enum HandleRCDelta<T: MetaDataRenderAsset> {
    Add(RenderAssetHandle<T>),
    Remove(RenderAssetHandle<T>),
}

/// A handle to a render asset
#[derive(Debug)]
pub enum RenderAssetHandle<T: MetaDataRenderAsset> {
    Strong {
        handle: Slot<AssetHandle<T::Asset>>,
        dropped_handles_send: crossbeam_channel::Sender<HandleRCDelta<T>>,
    },
    Weak {
        handle: Slot<AssetHandle<T::Asset>>,
    },
}
impl<T: MetaDataRenderAsset> AsRef<Slot<AssetHandle<T::Asset>>> for RenderAssetHandle<T> {
    fn as_ref(&self) -> &Slot<AssetHandle<T::Asset>> {
        match self {
            RenderAssetHandle::Strong { handle, .. } => handle,
            RenderAssetHandle::Weak { handle } => handle,
        }
    }
}
impl<T: MetaDataRenderAsset> Deref for RenderAssetHandle<T> {
    type Target = Slot<AssetHandle<T::Asset>>;

    fn deref(&self) -> &Self::Target {
        match &self {
            RenderAssetHandle::Strong { handle, .. } => handle,
            RenderAssetHandle::Weak { handle, .. } => handle,
        }
    }
}
impl<T: MetaDataRenderAsset> Clone for RenderAssetHandle<T> {
    fn clone(&self) -> Self {
        match &self {
            RenderAssetHandle::Strong {
                handle,
                dropped_handles_send,
            } => {
                let clone = RenderAssetHandle::Strong {
                    handle: handle.clone(),
                    dropped_handles_send: dropped_handles_send.clone(),
                };
                dropped_handles_send
                    .send(HandleRCDelta::Add(RenderAssetHandle::Weak {
                        handle: handle.clone(),
                    }))
                    .unwrap();
                clone
            }
            RenderAssetHandle::Weak { handle } => RenderAssetHandle::Weak {
                handle: handle.clone(),
            },
        }
    }
}
impl<T: MetaDataRenderAsset> Drop for RenderAssetHandle<T> {
    fn drop(&mut self) {
        match self {
            RenderAssetHandle::Strong {
                handle,
                dropped_handles_send,
            } => {
                dropped_handles_send.send(HandleRCDelta::Remove(RenderAssetHandle::Weak {
                    handle: handle.clone(),
                }));
            }
            RenderAssetHandle::Weak { .. } => {}
        }
    }
}
impl<T: MetaDataRenderAsset> PartialEq for RenderAssetHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl<T: MetaDataRenderAsset> Eq for RenderAssetHandle<T> {}
impl<T: MetaDataRenderAsset> Hash for RenderAssetHandle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state);
    }
}
