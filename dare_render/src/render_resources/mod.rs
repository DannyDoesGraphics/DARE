use bevy_ecs::prelude::*;
use std::sync::atomic::Ordering;

mod buffer_stream;
mod dirty_map;
use dare_ecs::SubAppMainLabel;
pub use dirty_map::*;

use crate::RenderSubAppLabel;

/// Handles per tick updating of resources that have gone unused or used
pub fn render_assets(
    project_mappings: Res<dare_ecs::ProjectEntityMapping<SubAppMainLabel, RenderSubAppLabel>>,
    meshes: Res<dare_assets::AssetsProjection<dare_assets::Mesh>>,
    buffers: Res<dare_assets::AssetsProjection<dare_assets::Buffer>>,
    mesh_query: Query<(
        &dare_assets::AssetHandle<dare_assets::Mesh>,
        &dare_physics::Transform,
    )>,
    transfer_belt: Res<crate::transfer_belt::TransferPool>,
    visible_meshes: Res<crate::plugin::VisibleMeshList>,
) {
    for entity in visible_meshes.0.iter() {
        let entity = project_mappings
            .get(entity)
            .expect("Expected mapping, got None");
        let (mesh_handle, _) = mesh_query.get(entity).unwrap();
        let mesh_runtime = meshes.runtime(mesh_handle).unwrap();

        let should_load: bool = mesh_runtime
            .residency
            .compare_exchange(
                *dare_assets::ResidentState::Unloaded,
                *dare_assets::ResidentState::Loading,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok();
        if should_load {
            // send to io pool to handle it
            //transfer_belt.enqueue();
        }
    }

    for (handle, runtime) in buffers.iter_runtimes() {
        if runtime.residency.load(Ordering::Acquire) == *dare_assets::ResidentState::ResidentGPU {
            // SAFETY: failed fetches literally just mean we are at ttl == 0 already.
            unsafe {
                runtime
                    .ttl
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |ttl| {
                        if ttl == 1 {
                            runtime
                                .residency
                                .store(*dare_assets::ResidentState::Unloading, Ordering::Relaxed);
                        }
                        ttl.checked_sub(1)
                    })
                    .unwrap_unchecked();
            }
        }
    }
}
