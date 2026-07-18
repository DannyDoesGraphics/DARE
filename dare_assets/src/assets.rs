use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    marker::PhantomData,
    ops::Deref,
    sync::Arc,
};

use crate::AssetHandle;
use bevy_ecs::resource::Resource;
use dare_containers::slot_map::SlotMap;

/// Describes where the underlying bytes are located.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataLocation {
    Url {
        url: String,
        offset: usize,
        length: usize,
    },
    File {
        path: std::path::PathBuf,
        offset: usize,
        length: usize,
    },
}

impl DataLocation {
    /// We generate a task which runs in the background thread which is responsible for handling
    ///
    /// [`buffer_size`] parameter is not guaranteed to be always fit within and should be assumed to
    /// overflow at any moment. The primary purpose of [`buffer_size`].
    ///
    /// # Cases and conditions
    /// Covers different data locations, and how they stream
    ///
    /// ## [`DataLocation::Url`]
    /// No guarantees on each item's length and size
    ///
    /// ## [`DataLocation::File`]
    /// Ensures that each byte sent in stream must be <= buffer_size
    pub async fn generate_stream(
        &self,
        buffer_size: u64,
    ) -> anyhow::Result<impl futures::Stream<Item = anyhow::Result<Box<[u8]>>>> {
        use futures::StreamExt;
        use std::io::{Read, Seek};
        match self {
            Self::Url {
                url,
                offset,
                length,
            } => {
                let agent = ureq::agent();
                let url = url.clone();
                let range_header = format!("bytes={}-{}", offset, length + offset);
                let offset = *offset;
                let length = *length;
                let buffer_size = (buffer_size as usize).max(1);
                let (sender, receiver) = futures::channel::mpsc::unbounded();

                smol::unblock(move || {
                    let run = || -> anyhow::Result<()> {
                        let response = agent.get(&url).header("Range", range_header).call()?;
                        // is this server actually good (or cringe)
                        let server_honored: bool =
                            response.status() == ureq::http::StatusCode::PARTIAL_CONTENT;
                        let mut skip: usize = if server_honored { 0 } else { offset };
                        let mut take: usize = length;
                        let mut reader = response.into_body().into_reader();
                        let mut buf = vec![0u8; buffer_size];
                        // this loop exists as a way for us to minimize the # of empty chunk
                        // sends due to byte skipping.
                        while take > 0 {
                            let n = reader.read(&mut buf)?;
                            if n == 0 {
                                break;
                            }
                            let mut slice = &buf[..n];
                            if skip > 0 {
                                if slice.len() <= skip {
                                    skip -= slice.len();
                                    continue;
                                }
                                slice = &slice[skip..];
                                skip = 0;
                            }
                            let to_take = slice.len().min(take);
                            let boxed_chunk: Box<[u8]> = slice[..to_take].into();
                            take -= to_take;
                            if sender.unbounded_send(Ok(boxed_chunk)).is_err() {
                                return Ok(());
                            }
                        }
                        Ok(())
                    };
                    if let Err(e) = run() {
                        let _ = sender.unbounded_send(Err(e));
                    }
                })
                .detach();

                Ok(receiver.boxed())
            }
            Self::File {
                path,
                offset,
                length,
            } => {
                let path = path.clone();
                let offset = *offset;
                let mut remaining = *length;
                let buffer_size = (buffer_size as usize).max(1);
                let (sender, receiver) = futures::channel::mpsc::unbounded();

                smol::unblock(move || {
                    let mut run = || -> anyhow::Result<()> {
                        let mut file = std::fs::File::open(&path)?;
                        file.seek(std::io::SeekFrom::Start(offset as u64))?;
                        let mut buf = vec![0u8; buffer_size];
                        while remaining > 0 {
                            let max_take = buf.len().min(remaining);
                            let n = file.read(&mut buf[..max_take])?;
                            if n == 0 {
                                break;
                            }
                            remaining -= n;
                            let chunk: Box<[u8]> = buf[..n].into();
                            if sender.unbounded_send(Ok(chunk)).is_err() {
                                return Ok(());
                            }
                        }
                        Ok(())
                    };
                    if let Err(e) = run() {
                        let _ = sender.unbounded_send(Err(e));
                    }
                })
                .detach();

                Ok(receiver.boxed())
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
#[repr(u8)]
pub enum ResidentState {
    /// Asset is not resident on the GPU
    Empty = 0u8,
    /// Asset is being loaded onto the GPU
    Loading = 1u8,
    /// Asset is resident on the GPU, and is ready to be used
    ResidentGPU = 2u8,
    /// Asset is being unloaded from the GPU
    Unloading = 3u8,
    /// Asset is no longer resident on the GPU
    Unloaded = 4u8,
    /// Asset failed to load, and must be manually acknowledged by the user
    Failed = 5u8,
}
impl Deref for ResidentState {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        unsafe { &*std::ptr::from_ref(self).cast::<u8>() }
    }
}

/// Always represents an instance of [`Asset`], and is backed by every [`crate::AssetHandle<T>`]
///
/// Defines the resident state of geometries
#[derive(Debug)]
pub struct AssetRuntime {
    /// See [`ResidentState`]
    pub residency: std::sync::atomic::AtomicU8,
    /// Time to live remaining on asset
    pub ttl: std::sync::atomic::AtomicU16,
    pub max_ttl: u16,
    /// Debug name, set once at insert and never mutated afterward
    pub name: Option<String>,
}

impl Default for AssetRuntime {
    /// By default, constructs a runtime that will be destroyed instantly, it is expected you set the TTL remaining
    fn default() -> Self {
        Self {
            residency: std::sync::atomic::AtomicU8::from(0),
            ttl: std::sync::atomic::AtomicU16::from(0),
            max_ttl: 0,
            name: None,
        }
    }
}

impl AssetRuntime {
    pub fn touch(&self) {
        self.ttl
            .store(self.max_ttl, std::sync::atomic::Ordering::Relaxed);
    }
}

/// An asset which can be uploaded onto the GPU
pub trait Asset: Clone + Debug + Send + Sync + Sized + 'static {
    type GpuResource: Debug + Send + Sync + 'static;
}

/// An asset container which holds runtime state of each asset.
///
/// # Cross-thread synchronization
/// To handle assets from the engine to other worlds in different threads such as the rendering thread, we use [`AssetSync<A>`] to allow
/// for **projection** between world A to B. This typically means that the engine world serves as the ground source truth.
#[derive(Debug, Resource, Default)]
pub struct Assets<A: Asset> {
    slot_map: SlotMap<A, AssetHandle<A>>,
    /// Refers to the set of handles which have not been properly acknowledged
    dirty_set: HashSet<AssetHandle<A>>,
    runtime_state: HashMap<AssetHandle<A>, Arc<AssetRuntime>>,
    ttl: u16,
}

impl<A: Asset> Assets<A> {
    pub fn new(ttl: u16) -> Self {
        Self {
            slot_map: SlotMap::default(),
            dirty_set: HashSet::default(),
            runtime_state: HashMap::default(),
            ttl,
        }
    }

    pub fn insert(&mut self, asset: A) -> AssetHandle<A> {
        self.insert_named(asset, None::<String>)
    }

    pub fn insert_named(&mut self, asset: A, name: Option<impl Into<String>>) -> AssetHandle<A> {
        let handle = self.slot_map.insert(asset);
        let runtime = AssetRuntime {
            residency: std::sync::atomic::AtomicU8::from(*ResidentState::Unloaded),
            ttl: std::sync::atomic::AtomicU16::from(self.ttl),
            max_ttl: self.ttl,
            name: name.map(Into::into),
        };
        self.runtime_state.insert(handle.clone(), Arc::new(runtime));
        self.dirty_set.insert(handle.clone());

        handle
    }

    pub fn remove(&mut self, handle: AssetHandle<A>) -> Option<A> {
        self.runtime_state.remove(&handle);
        self.dirty_set.insert(handle.clone());
        self.slot_map.remove(handle).ok()
    }

    pub fn update(&mut self, handle: &AssetHandle<A>, asset: A) -> Option<A> {
        let runtime = self.runtime_state.get(handle).unwrap();
        runtime.touch();
        let mut asset: A = asset;
        self.slot_map.get_mut(handle).map(|slot| {
            self.dirty_set.insert(handle.clone());
            std::mem::swap(slot, &mut asset);
            asset
        })
    }

    pub fn iter_runtimes(
        &self,
    ) -> impl Iterator<Item = (&crate::AssetHandle<A>, &Arc<crate::AssetRuntime>)> {
        self.runtime_state.iter()
    }

    pub fn get_mut(&mut self, handle: &AssetHandle<A>) -> Option<&mut A> {
        self.dirty_set.insert(handle.clone());
        self.slot_map.get_mut(handle)
    }

    pub fn get(&self, handle: &AssetHandle<A>) -> Option<&A> {
        self.slot_map.get(handle)
    }

    pub fn get_runtime(&self, handle: &AssetHandle<A>) -> Option<&Arc<AssetRuntime>> {
        self.runtime_state.get(handle)
    }
}

#[derive(Debug, Resource)]
pub struct AssetsProjection<A: Asset> {
    assets: HashMap<AssetHandle<A>, A>,
    runtime_state: HashMap<AssetHandle<A>, Arc<AssetRuntime>>,
}

impl<A: Asset> Default for AssetsProjection<A> {
    fn default() -> Self {
        Self {
            assets: HashMap::default(),
            runtime_state: HashMap::default(),
        }
    }
}

impl<A: Asset> AssetsProjection<A> {
    pub fn get(&self, handle: &AssetHandle<A>) -> Option<&A> {
        self.assets.get(handle)
    }

    pub fn runtime(&self, handle: &AssetHandle<A>) -> Option<&Arc<AssetRuntime>> {
        self.runtime_state.get(handle)
    }

    pub fn contains(&self, handle: &AssetHandle<A>) -> bool {
        self.assets.contains_key(handle)
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AssetHandle<A>, &A)> {
        self.assets.iter()
    }

    pub fn iter_runtimes(
        &self,
    ) -> impl Iterator<Item = (&crate::AssetHandle<A>, &Arc<crate::AssetRuntime>)> {
        self.runtime_state.iter()
    }

    fn upsert(&mut self, handle: AssetHandle<A>, asset: A, runtime: Arc<AssetRuntime>) {
        self.assets.insert(handle.clone(), asset);
        self.runtime_state.insert(handle, runtime);
    }

    fn remove(&mut self, handle: &AssetHandle<A>) {
        self.assets.remove(handle);
        self.runtime_state.remove(handle);
    }
}

/// Asset sync effectively perform a hashmap projection from `From` to `To`
///
/// # Limitations
/// A large restriction to consider is that our hashmap only allows **at most one** projection per asset.
pub struct AssetSync<A: Asset, From: dare_ecs::SubAppLabel, To: dare_ecs::SubAppLabel> {
    ttl: u16,
    _marker: PhantomData<(A, From, To)>,
}

impl<A: Asset, From: dare_ecs::SubAppLabel, To: dare_ecs::SubAppLabel> AssetSync<A, From, To> {
    pub fn new(ttl: u16) -> Self {
        Self {
            ttl,
            _marker: PhantomData,
        }
    }
}

// TODO: support updating so we don't spam runtime labels
enum AssetDelta<A: Asset> {
    Upserted {
        handle: AssetHandle<A>,
        asset: A,
        runtime: Arc<AssetRuntime>,
    },
    Removed {
        handle: AssetHandle<A>,
    },
}

impl<A: Asset, From: dare_ecs::SubAppLabel, To: dare_ecs::SubAppLabel> dare_ecs::Plugin
    for AssetSync<A, From, To>
{
    fn build(&self, app: &mut dare_ecs::App) {
        if app
            .get_sub_app::<From>()
            .unwrap()
            .world()
            .get_resource::<Assets<A>>()
            .is_none()
        {
            app.get_sub_app_mut::<From>()
                .unwrap()
                .world_mut()
                .insert_resource(Assets::<A>::new(self.ttl));
        }
        if app
            .get_sub_app::<To>()
            .unwrap()
            .world()
            .get_resource::<AssetsProjection<A>>()
            .is_none()
        {
            app.get_sub_app_mut::<To>()
                .unwrap()
                .world_mut()
                .insert_resource(AssetsProjection::<A>::default());
        }
        use bevy_ecs::prelude::*;

        app.add_plugin(
            dare_ecs::ExtractPlugin::<Vec<AssetDelta<A>>, To, From>::new(
                |world: &mut World| {
                    let mut assets = world.get_resource_mut::<Assets<A>>()?;
                    let handles: Vec<_> = assets.dirty_set.drain().collect();
                    if handles.is_empty() {
                        return None;
                    }
                    let deltas: Vec<AssetDelta<A>> = handles
                        .into_iter()
                        .map(|handle| {
                            if let Some(asset) = assets.slot_map.get(&handle) {
                                let runtime = assets.runtime_state.get(&handle).unwrap().clone();
                                AssetDelta::Upserted {
                                    handle,
                                    asset: asset.clone(),
                                    runtime,
                                }
                            } else {
                                AssetDelta::Removed { handle }
                            }
                        })
                        .collect();

                    if deltas.is_empty() {
                        None
                    } else {
                        Some(deltas)
                    }
                },
                |world: &mut World, deltas: Vec<Vec<AssetDelta<A>>>| {
                    let mut assets = world.get_resource_mut::<AssetsProjection<A>>().unwrap();
                    for deltas in deltas {
                        for delta in deltas {
                            match delta {
                                AssetDelta::Upserted {
                                    handle,
                                    asset,
                                    runtime,
                                } => {
                                    assets.upsert(handle, asset, runtime);
                                }
                                AssetDelta::Removed { handle } => {
                                    assets.remove(&handle);
                                }
                            }
                        }
                    }
                },
            ),
        );
    }
}
