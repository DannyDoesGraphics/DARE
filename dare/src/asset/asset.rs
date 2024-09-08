use anyhow::Result;
use futures::stream::BoxStream;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

/// Holds an asset and tracks its loaded state and metadata
#[derive(Debug)]
pub struct AssetHolder<A: AssetDescriptor> {
    pub metadata: A::Metadata,
    pub state: Arc<RwLock<AssetState<A>>>,
}

impl<A: AssetDescriptor> Clone for AssetHolder<A> {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            state: self.state.clone(),
        }
    }
}

impl<A: AssetDescriptor> PartialEq for AssetHolder<A> {
    fn eq(&self, other: &Self) -> bool {
        self.metadata == other.metadata
    }
}
impl<A: AssetDescriptor> Eq for AssetHolder<A> {}

impl<A: AssetDescriptor + PartialEq> AssetHolder<A> {
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

pub trait AssetUnloaded: Hash + PartialEq + Eq + Clone {
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
}

/// Describes the possible location of the files
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum MetaDataLocation {
    FilePath(std::path::PathBuf),
    Link(String),
}
