use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    marker::PhantomData,
    ops::Deref,
    sync::Arc,
};

use bevy_ecs::resource::Resource;
use dare_containers::slot_map::SlotMap;

use crate::AssetHandle;

/// Describes where the underlying bytes are located.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataLocation {
    Url(String),
    File(std::path::PathBuf),
    Blob(Arc<[u8]>),
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
}

impl Default for AssetRuntime {
    /// By default, constructs a runtime that will be destroyed instantly, it is expected you set the ttl remaining
    fn default() -> Self {
        Self {
            residency: std::sync::atomic::AtomicU8::from(0),
            ttl: std::sync::atomic::AtomicU16::from(0),
        }
    }
}

/// An asset which can be uploaded onto the GPU
pub trait Asset: Clone + Debug + Send + Sync + Sized + 'static {}

/// An asset container which holds runtime state of each asset.
///
/// # Cross-thread synchronization
/// To handle assets from the engine to other worlds in different threads such as the rendering thread, we use [`AssetSync<A>`] to allow
/// for **projection** between world A to B. This typically means that the engine world serves as the ground source truth.
#[derive(Debug, Resource, Default)]
pub struct Assets<A: Asset> {
    slot_map: SlotMap<A, AssetHandle<A>>,
    /// Refers to the set of handles which have not been proeprly acknowledged
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
        let handle = self.slot_map.insert(asset);
        self.runtime_state
            .insert(handle.clone(), Arc::new(AssetRuntime::default()));
        let runtime = self.runtime_state.get_mut(&handle).unwrap();
        runtime.residency.store(
            *ResidentState::Unloaded,
            std::sync::atomic::Ordering::Release,
        );
        runtime
            .ttl
            .store(self.ttl, std::sync::atomic::Ordering::Release);
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
        runtime
            .ttl
            .store(self.ttl, std::sync::atomic::Ordering::Release);
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
                            if let Some(asset) = assets.slot_map.get(handle.clone()) {
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
