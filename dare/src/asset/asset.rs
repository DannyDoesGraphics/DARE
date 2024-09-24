use anyhow::Result;
use futures::stream::BoxStream;
use std::fmt;
use std::fmt::{Debug, Formatter, Pointer};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

pub trait MetadataGetter<A: AssetDescriptor> {
    /// Get reference to the asset's metadata
    fn get_metadata(&self) -> &A::Metadata;
}

/// Simply only contains an asset's metadata as well it's weak reference
pub struct WeakAssetRef<A: AssetDescriptor> {
    pub metadata: A::Metadata,
    pub state: Weak<A::Loaded>,
}

impl<A: AssetDescriptor> Clone for WeakAssetRef<A> {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            state: self.state.clone(),
        }
    }
}

impl<A: AssetDescriptor> Debug for WeakAssetRef<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WeakAssetRef")
            .field("metadata", &self.metadata)
            .field("state", &self.state.upgrade())
            .finish()
    }
}

impl<A: AssetDescriptor> MetadataGetter<A> for WeakAssetRef<A> {
    fn get_metadata(&self) -> &A::Metadata {
        &self.metadata
    }
}

impl<A: AssetDescriptor> WeakAssetRef<A> {
    pub fn upgrade(&self) -> Option<StrongAssetRef<A>> {
        Some(StrongAssetRef {
            metadata: self.metadata.clone(),
            state: Weak::upgrade(&self.state)?,
        })
    }
}

/// Only contains an asset's metadata as well it's strong reference
pub struct StrongAssetRef<A: AssetDescriptor> {
    pub metadata: A::Metadata,
    pub state: Arc<A::Loaded>,
}

impl<A: AssetDescriptor> MetadataGetter<A> for StrongAssetRef<A> {
    fn get_metadata(&self) -> &A::Metadata {
        &self.metadata
    }
}

impl<A: AssetDescriptor> Clone for StrongAssetRef<A> {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            state: self.state.clone(),
        }
    }
}

impl<A: AssetDescriptor> Debug for StrongAssetRef<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("WeakAssetRef")
            .field("metadata", &self.metadata)
            .field("state", &self.state)
            .finish()
    }
}

/// Holds an asset and tracks its loaded state and metadata
#[derive(Debug)]
pub struct AssetMetadataAndState<A: AssetDescriptor> {
    pub metadata: A::Metadata,
    pub state: Arc<RwLock<AssetState<A>>>,
}

impl<A: AssetDescriptor> Clone for AssetMetadataAndState<A> {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            state: self.state.clone(),
        }
    }
}

impl<A: AssetDescriptor> PartialEq for AssetMetadataAndState<A> {
    fn eq(&self, other: &Self) -> bool {
        self.metadata == other.metadata
    }
}
impl<A: AssetDescriptor> Eq for AssetMetadataAndState<A> {}

impl<A: AssetDescriptor> Hash for AssetMetadataAndState<A> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.metadata.hash(state);
    }
}

impl<A: AssetDescriptor> AssetMetadataAndState<A> {
    pub fn new(metadata: A::Metadata) -> Self {
        Self {
            metadata: metadata.clone(),
            state: Arc::new(RwLock::new(AssetState::Unloaded(metadata))),
        }
    }
}

pub trait AssetDescriptor {
    type Loaded: PartialEq + Eq + Debug;
    /// Any data as to how the asset should be loaded in
    type Metadata: AssetUnloaded<AssetLoaded = Self::Loaded>;
}

pub trait AssetUnloaded: Hash + PartialEq + Eq + Clone + Debug {
    type AssetLoaded;
    type Chunk;
    type StreamInfo;
    type LoadInfo;

    /// Streams the data in pre-defined chunks
    async fn stream(
        self,
        stream_info: Self::StreamInfo,
    ) -> Result<BoxStream<'static, Result<Self::Chunk>>>;

    /// Simply loads an asset directly to the gpu
    async fn load(
        &self,
        load_info: Self::LoadInfo,
        sender: tokio::sync::watch::Sender<Option<Arc<Self::AssetLoaded>>>,
    ) -> Result<Arc<Self::AssetLoaded>>;
}

#[derive(Debug)]
pub enum AssetState<A: AssetDescriptor> {
    Unloaded(A::Metadata),
    Loading(tokio::sync::watch::Receiver<Option<Arc<A::Loaded>>>),
    Loaded(Arc<A::Loaded>),
    Unloading(Weak<A::Loaded>),
}

impl<A: AssetDescriptor> AssetState<A> {
    pub fn unload(self) -> Self {
        match self {
            AssetState::Loaded(loading) => AssetState::Unloading(Arc::downgrade(&loading)),
            _ => unimplemented!(),
        }
    }

    pub async fn load(
        &mut self,
        load_info: <<A as AssetDescriptor>::Metadata as AssetUnloaded>::LoadInfo,
    ) -> Result<Self> {
        match self {
            AssetState::Unloaded(metadata) => {
                let metadata = metadata.clone();
                let (send, recv) = tokio::sync::watch::channel(None);
                *self = Self::Loading(recv);
                let loaded = metadata.load(load_info, send).await?;
                Ok(Self::Loaded(loaded))
            }
            _ => unimplemented!(),
        }
    }

    /// Get a ref to the underlying asset being held
    pub async fn get_asset(&self) -> Option<Arc<A::Loaded>> {
        match self {
            AssetState::Loaded(loaded) => Some(loaded.clone()),
            _ => None,
        }
    }
}

/// Describes the possible location of the files
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum MetaDataLocation {
    /// Data is behind a physical file that must be loaded
    FilePath(std::path::PathBuf),
    /// Data is behind a link
    Link(String),
    /// Describes the data is held in memory
    Memory(Arc<[u8]>),
}
