use bevy_ecs::prelude::*;

mod buffer_stream;
mod dirty_map;
pub use dirty_map::*;

/// Handles per tick updating of resources that have gone unused or used
pub fn render_assets(
    buffer_handles: Query<(&dare_assets::AssetHandle<dare_assets::Buffer>)>,
    buffers: Res<dare_assets::Assets<dare_assets::Buffer>>,
    meshes: Res<dare_assets::Assets<dare_assets::Mesh>>,
) {
    for (handle, runtime) in buffers.iter_runtimes() {
        // if an asset goes unused s.t. there ttl <= 0, remove it from the GPU
    }
}
