use bevy_ecs::prelude::*;
use glm::intBitsToFloat;
use dagal::allocators::{GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use crate::asset2::server::AssetServerDelta;
use crate::prelude as dare;

pub fn asset_manager_system(rt: Res<dare::concurrent::BevyTokioRunTime>, render_context: Res<dare::render::contexts::RenderContext>,mut buffer_storage: ResMut<super::RenderAssetManagerStorage<dare::render::components::RenderBuffer<GPUAllocatorImpl>>>) {

    rt.runtime.block_on(async move {
        let local_set = tokio::task::LocalSet::new();
        for delta in buffer_storage.asset_server.get_deltas() {
            match delta {
                AssetServerDelta::HandleCreated(untyped_handle) => {}
                AssetServerDelta::HandleLoading(untyped_handle) => {
                    let asset_id = untyped_handle.get_id();
                    if let Some(handle) = untyped_handle.into_typed_handle::<dare::asset2::assets::Buffer>() {
                        match buffer_storage.insert(handle.clone()).map_err(|e| {
                            tracing::error!("Failed to insert handle {e}")
                        }) {
                            Err(e) => {},
                            Ok(_) => {
                                tracing::trace!("Loading incoming handle {:?}", asset_id);
                                if let Some(asset_storage_handle) = buffer_storage.get_storage_handle(&handle) {
                                    if let Some(buffer_metadata) = buffer_storage.asset_server.get_metadata(&handle) {
                                        buffer_storage.load(&asset_storage_handle, dare::render::render_assets::components::BufferPrepareInfo {
                                            allocator: render_context.inner.allocator.clone(),
                                            handle,
                                            transfer_pool: render_context.transfer_pool(),
                                            usage_flags: vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                                            location: MemoryLocation::GpuOnly,
                                            name: Some(buffer_metadata.name),
                                        }, dare::asset2::assets::BufferStreamInfo {
                                            chunk_size: render_context.transfer_pool().cpu_staging_size() as usize,
                                        }, &local_set);
                                    }
                                }
                            }
                        }
                    }
                }
                AssetServerDelta::HandleUnloading(untyped_handle) => {
                    // remove a reference to indicate we no longer need it
                    if let Some(handle) = untyped_handle.into_typed_handle::<dare::asset2::assets::Buffer>() {
                        if let Some(render_asset_handle) = buffer_storage.get_storage_handle(&handle) {
                            buffer_storage.handle_references.get_mut(&*render_asset_handle).map(|mut v| {
                                *v -= 1;
                            });
                        }
                    }
                }
                AssetServerDelta::HandleDestroyed(_) => {}
            }
        }
        // finish awaiting load tasks
        local_set.await;
        buffer_storage.process_queue();
    });
}