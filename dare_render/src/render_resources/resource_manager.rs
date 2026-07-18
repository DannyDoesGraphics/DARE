use bevy_ecs::prelude::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use dare_ecs::{AppStage, SubAppLabel};
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::task::{Context, Poll, Waker};

#[derive(Debug)]
pub struct ResourceEntry<A: dare_assets::Asset> {
    pub resource: dare_util::Either<A::GpuResource, smol::Task<anyhow::Result<A::GpuResource>>>,
}

impl<A: dare_assets::Asset> ResourceEntry<A> {
    #[allow(dead_code)]
    pub fn is_gpu(&self) -> bool {
        self.resource.is_left()
    }

    #[allow(dead_code)]
    pub fn is_task(&self) -> bool {
        !self.resource.is_left()
    }
}

#[derive(Debug, Resource)]
pub struct GpuResourceManager<A: dare_assets::Asset> {
    map: HashMap<dare_assets::AssetHandle<A>, ResourceEntry<A>>,
}

impl<A: dare_assets::Asset> GpuResourceManager<A> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Insert a task which will be periodically noop polled
    pub fn insert_task(
        &mut self,
        handle: dare_assets::AssetHandle<A>,
        task: smol::Task<anyhow::Result<A::GpuResource>>,
    ) -> Option<ResourceEntry<A>> {
        self.map.insert(
            handle,
            ResourceEntry {
                resource: dare_util::Either::Right(task),
            },
        )
    }

    /// Tick for any finished tasks and convert instantly to their GPU resource counterparts
    pub fn tick(&mut self, assets: &dare_assets::AssetsProjection<A>) {
        let mut cx = Context::from_waker(Waker::noop());
        let mut failed: Vec<dare_assets::AssetHandle<A>> = Vec::new();

        for (handle, entry) in self.map.iter_mut() {
            let ready = match &mut entry.resource {
                dare_util::Either::Left(_) => None,
                dare_util::Either::Right(task) => match Pin::new(task).poll(&mut cx) {
                    Poll::Ready(result) => Some(result),
                    Poll::Pending => None,
                },
            };
            match ready {
                Some(Ok(resource)) => {
                    // asset load task is done, we so mark it as loaded
                    entry.resource = dare_util::Either::Left(resource);
                    match assets.runtime(handle) {
                        Some(runtime) => {
                            runtime
                                .residency
                                .store(*dare_assets::ResidentState::ResidentGPU, Ordering::Relaxed);
                            tracing::debug!(name = ?runtime.name, "Imported {:?}", handle);
                        }
                        None => failed.push(handle.clone()),
                    }
                }
                Some(Err(err)) => {
                    tracing::error!(?err, "Failed to upload GPU resource");
                    failed.push(handle.clone());
                }
                None => {
                    // if asset needs to be unloaded, we unload it here and mark it as such
                    if let Some(runtime) = assets.runtime(handle)
                        && runtime.residency.load(Ordering::Acquire)
                            == *dare_assets::ResidentState::Unloading
                    {
                        tracing::debug!("Unloaded {:?}", handle);
                        failed.push(handle.clone());
                        runtime.residency.store(
                            *dare_assets::ResidentState::Unloaded,
                            std::sync::atomic::Ordering::Relaxed,
                        );
                    }
                }
            }
        }

        for handle in failed {
            self.map.remove(&handle);
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn get(&self, handle: &dare_assets::AssetHandle<A>) -> Option<&A::GpuResource> {
        self.map.get(handle)?.resource.left_ref()
    }

    pub fn contains(&self, handle: &dare_assets::AssetHandle<A>) -> bool {
        self.map.contains_key(handle)
    }
}

impl<A: dare_assets::Asset> AsRef<HashMap<dare_assets::AssetHandle<A>, ResourceEntry<A>>>
    for GpuResourceManager<A>
{
    fn as_ref(&self) -> &HashMap<dare_assets::AssetHandle<A>, ResourceEntry<A>> {
        &self.map
    }
}

impl<A: dare_assets::Asset> AsMut<HashMap<dare_assets::AssetHandle<A>, ResourceEntry<A>>>
    for GpuResourceManager<A>
{
    fn as_mut(&mut self) -> &mut HashMap<dare_assets::AssetHandle<A>, ResourceEntry<A>> {
        &mut self.map
    }
}

fn gpu_resource_manager_tick_system<A: dare_assets::Asset>(
    mut manager: ResMut<GpuResourceManager<A>>,
    assets: Res<dare_assets::AssetsProjection<A>>,
) {
    manager.tick(&assets);
}

fn decay_ttl_system<A: dare_assets::Asset>(assets: Res<dare_assets::AssetsProjection<A>>) {
    for (_handle, runtime) in assets.iter_runtimes() {
        if runtime.residency.load(Ordering::Acquire) != *dare_assets::ResidentState::ResidentGPU {
            continue;
        }
        let ttl = runtime.ttl.load(Ordering::Relaxed);
        let next_ttl = ttl.saturating_sub(1);
        let _ = runtime
            .ttl
            .compare_exchange(ttl, next_ttl, Ordering::Relaxed, Ordering::Relaxed);
        if ttl == 1 {
            let _ = runtime.residency.compare_exchange(
                *dare_assets::ResidentState::ResidentGPU,
                *dare_assets::ResidentState::Unloading,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }
}

pub struct GpuAssetLifecyclePlugin<Sub: SubAppLabel, A: dare_assets::Asset> {
    _marker: PhantomData<(Sub, A)>,
}

impl<Sub: SubAppLabel, A: dare_assets::Asset> GpuAssetLifecyclePlugin<Sub, A> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<Sub: SubAppLabel, A: dare_assets::Asset> Default for GpuAssetLifecyclePlugin<Sub, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Sub: SubAppLabel + 'static, A: dare_assets::Asset> dare_ecs::Plugin
    for GpuAssetLifecyclePlugin<Sub, A>
{
    fn build(&self, app: &mut dare_ecs::App) {
        let sub_app = app
            .get_sub_app_mut::<Sub>()
            .expect("GpuAssetLifecyclePlugin: target sub-app must be registered first");

        if sub_app
            .world()
            .get_resource::<GpuResourceManager<A>>()
            .is_none()
        {
            sub_app
                .world_mut()
                .insert_resource(GpuResourceManager::<A>::new());
        }

        sub_app.schedule_scope(|schedule| {
            schedule.add_systems(
                (
                    decay_ttl_system::<A>,
                    gpu_resource_manager_tick_system::<A>.after(decay_ttl_system::<A>),
                )
                    .in_set(AppStage::PreUpdate),
            );
        });
    }
}
