use bevy_ecs::prelude::*;
use std::sync::atomic::Ordering;

mod buffer_stream;
mod dirty_map;
mod resource_manager;
use dagal::{ash::vk, resource::traits::Resource};
use dare_ecs::SubAppMainLabel;
use futures::StreamExt;

use crate::RenderSubAppLabel;
pub use resource_manager::*;

const BUFFER_SIZE: usize = 2usize.pow(16);
/// Handles per tick updating of **meshes** and their respective buffers ensuring they remain alive
#[allow(clippy::too_many_arguments)]
pub fn render_assets(
    gpu_context: NonSend<crate::contexts::RenderGpu<dagal::allocators::GPUAllocatorImpl>>,
    project_mappings: Res<dare_ecs::ProjectEntityMapping<SubAppMainLabel, RenderSubAppLabel>>,
    meshes: Res<dare_assets::AssetsProjection<dare_assets::Mesh>>,
    buffers: Res<dare_assets::AssetsProjection<dare_assets::Buffer>>,
    mut gpu_buffers: ResMut<GpuResourceManager<dare_assets::Buffer>>,
    mesh_query: Query<(
        &dare_assets::AssetHandle<dare_assets::Mesh>,
        &dare_physics::Transform,
    )>,
    visible_meshes: Res<crate::plugin::VisibleMeshList>,
    task_pool: Res<dare_ecs::SmolExecutorHandle>,
) {
    for entity in visible_meshes.0.iter() {
        let entity = project_mappings
            .get(entity)
            .expect("Expected mapping, got None");
        let (mesh_handle, _) = mesh_query.get(entity).unwrap();
        let mesh_runtime = meshes.runtime(mesh_handle).unwrap();
        mesh_runtime.touch();

        if mesh_runtime.residency.load(Ordering::Acquire)
            == *dare_assets::ResidentState::ResidentGPU
        {
            let buffers_alive = meshes.get(mesh_handle).is_some_and(|mesh| {
                gpu_buffers.contains(&mesh.vertex_buffer)
                    && gpu_buffers.contains(&mesh.index_buffer)
            });
            if buffers_alive {
                if let Some(mesh) = meshes.get(mesh_handle) {
                    if let Some(rt) = buffers.runtime(&mesh.vertex_buffer) {
                        rt.touch();
                    }
                    if let Some(rt) = buffers.runtime(&mesh.index_buffer) {
                        rt.touch();
                    }
                }
                continue;
            }
            mesh_runtime
                .residency
                .store(*dare_assets::ResidentState::Unloaded, Ordering::Relaxed);
        }

        let should_load: bool = mesh_runtime
            .residency
            .compare_exchange(
                *dare_assets::ResidentState::Unloaded,
                *dare_assets::ResidentState::Loading,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok();
        // not synced yet, retry next tick
        let ready_mesh = should_load
            .then(|| meshes.get(mesh_handle))
            .flatten()
            .filter(|mesh| {
                buffers.get(&mesh.vertex_buffer).is_some()
                    && buffers.get(&mesh.index_buffer).is_some()
            });
        if should_load && ready_mesh.is_none() {
            mesh_runtime
                .residency
                .store(*dare_assets::ResidentState::Unloaded, Ordering::Relaxed);
        }
        if let Some(mesh) = ready_mesh {
            enum BufferType {
                Vertex,
                Index,
            }
            let mesh_buffers = [
                (
                    BufferType::Vertex,
                    mesh.vertex_buffer.clone(),
                    buffers.get(&mesh.vertex_buffer),
                ),
                (
                    BufferType::Index,
                    mesh.index_buffer.clone(),
                    buffers.get(&mesh.index_buffer),
                ),
            ];
            let transfer_belt = gpu_context.transfer_pool.clone();
            // handle buffers which need to be loaded in and load them
            mesh_buffers
                .into_iter()
                .filter_map(|(ty, buffer_handle, buffer)| {
                    buffer.map(|buffer| (ty, buffer_handle, buffer))
                })
                .for_each(|(buffer_type, buffer_handle, buffer)| {
                    // dedup: meshes can share a buffer
                    if gpu_buffers.contains(&buffer_handle) {
                        return;
                    }
                    let buffer = buffer.clone();
                    let target_format = match buffer_type {
                        BufferType::Index => dare_assets::Format::U32,
                        BufferType::Vertex => dare_assets::Format::F32x3,
                    };
                    let name = buffers.runtime(&buffer_handle).and_then(|r| r.name.clone());
                    let transfer_belt = transfer_belt.clone();
                    let mut gpu_buffer = dagal::resource::Buffer::new(
                        dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                            device: gpu_context.core.device.clone(),
                            name,
                            allocator: &gpu_context.core.allocator,
                            size: target_format.size_in_bytes() as u64 * buffer.count,
                            memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                            usage_flags: vk::BufferUsageFlags::INDEX_BUFFER
                                | vk::BufferUsageFlags::VERTEX_BUFFER
                                | vk::BufferUsageFlags::TRANSFER_SRC
                                | vk::BufferUsageFlags::TRANSFER_DST
                                | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                        },
                    )
                    .unwrap();
                    let task: smol::Task<
                        anyhow::Result<
                            dagal::resource::Buffer<dagal::allocators::GPUAllocatorImpl>,
                        >,
                    > = task_pool.spawn(async move {
                        let stream = buffer.location.generate_stream(BUFFER_SIZE as u64).await?;
                        let mut reshaper = dare_assets::ByteStreamReshaper::new(
                            stream,
                            buffer.format,
                            buffer.count,
                            Some(BUFFER_SIZE as u64 / target_format.size_in_bytes() as u64),
                            Some(target_format),
                        );

                        let mut dst_offset = 0u64;
                        while let Some(bytes) = reshaper.next().await {
                            let src_size = bytes.len() as u64;
                            let result = transfer_belt
                                .enqueue(crate::transfer_belt::TransferRequest::Buffer {
                                    dst_queue_family: None,
                                    buffer: gpu_buffer,
                                    dst_offset,
                                    src_size,
                                    data: bytes.into(),
                                })
                                .await
                                .unwrap();
                            gpu_buffer = result.into_buffer().unwrap();
                            dst_offset += src_size;
                        }
                        Ok(gpu_buffer)
                    });
                    gpu_buffers.insert_task(buffer_handle, task);
                })
        }
    }

    for (mesh_handle, mesh_runtime) in meshes.iter_runtimes() {
        if mesh_runtime.residency.load(Ordering::Acquire) != *dare_assets::ResidentState::Loading {
            continue;
        }
        let Some(mesh) = meshes.get(mesh_handle) else {
            continue;
        };
        let buffer_ready = |handle: &dare_assets::AssetHandle<dare_assets::Buffer>| {
            buffers.runtime(handle).is_some_and(|runtime| {
                runtime.residency.load(Ordering::Acquire)
                    == *dare_assets::ResidentState::ResidentGPU
            })
        };
        if buffer_ready(&mesh.vertex_buffer) && buffer_ready(&mesh.index_buffer) {
            mesh_runtime
                .residency
                .store(*dare_assets::ResidentState::ResidentGPU, Ordering::Relaxed);
            tracing::debug!(?mesh_handle, name = ?mesh_runtime.name, "Mesh loaded");
        }
    }
}
