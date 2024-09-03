use anyhow::Result;
use futures::stream::BoxStream;
use std::hash::Hash;
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

/// Holds an asset and tracks its loaded state and metadata
pub struct AssetHolder<A: AssetDescriptor> {
    pub metadata: A::Metadata,
    pub state: Arc<RwLock<AssetState<A>>>,
}

impl<A: AssetDescriptor + PartialEq> AssetHolder<A> {
    pub fn new(metadata: A::Metadata) -> Self {
        Self {
            metadata,
            state: Arc::new(RwLock::new(AssetState::Unloaded)),
        }
    }
}

pub trait AssetDescriptor {
    type Loaded: PartialEq;
    /// Any data as to how the asset should be loaded in
    type Metadata: AssetUnloaded<AssetLoaded=Self::Loaded>;
}

pub trait AssetUnloaded: Hash + PartialEq + Eq + Clone {
    type AssetLoaded;
    type Chunk;
    type StreamInfo;

    /// Streams the data in pre-defined chunks
    async fn stream(
        self,
        stream_info: Self::StreamInfo,
    ) -> Result<BoxStream<'static, Result<Self::Chunk>>>;
}

#[derive(Debug)]
pub enum AssetState<A: AssetDescriptor> {
    Unloaded,
    Loading(tokio::sync::watch::Receiver<Option<Arc<A::Loaded>>>),
    Loaded(Arc<A::Loaded>),
    Unloading(Weak<A::Loaded>),
}

impl<A: AssetDescriptor> AssetState<A> {
    pub fn unload(self) -> Self {
        match self {
            AssetState::Loaded(loading) => AssetState::Unloading(Arc::downgrade(&loading)),
            _ => unimplemented!()
        }
    }
}

/// Describes the possible location of the files
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum MetaDataLocation {
    FilePath(std::path::PathBuf),
    Link(String),
}
