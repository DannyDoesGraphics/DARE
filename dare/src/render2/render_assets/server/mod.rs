use crate::prelude as dare;
use crate::prelude::render::{InnerRenderServerRequest, RenderServerAssetRelationDelta};
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use crate::render2::server::IrRecv;
use bevy_ecs::prelude as becs;
use dagal::allocators::{GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use futures::stream::FuturesUnordered;
use futures::{StreamExt, TryFutureExt};
use std::sync::Arc;

/// Responsible for receiving and tracking with the main asset server
#[derive(becs::Resource, Clone)]
pub struct RenderAssetServer {
    asset_server: dare::asset2::server::AssetServer,
}

impl RenderAssetServer {
    pub fn new(asset_server: dare::asset2::server::AssetServer) -> Self {
        Self { asset_server }
    }
}

pub fn render_asset_server_system(
    mut commands: becs::Commands,
    mut buffers: becs::ResMut<
        '_,
        super::assets::RenderAssetsStorage<super::components::RenderBuffer<GPUAllocatorImpl>>,
    >,
    mut meshes: becs::ResMut<dare::render::resource_relationship::Meshes>,
    ir_recv: becs::ResMut<'_, IrRecv>,
    rt: becs::Res<'_, dare::concurrent::BevyTokioRunTime>,
    render_asset_server: becs::ResMut<'_, RenderAssetServer>,
    render_context: becs::Res<'_, dare::render::contexts::RenderContext>,
) {
    // Flush the asset server and handle any potential errors
    render_asset_server
        .asset_server
        .flush()
        .map_err(|err| {
            tracing::error!("{}", err);
        })
        .unwrap();
    // Offload to a blocking thread to handle potentially heavy asset loading
    let render_server = render_asset_server.clone();
    let render_context = render_context.clone();
    let buffer_server = buffers.server();

    {
        let mut futures = FuturesUnordered::new();
        for delta in render_server.asset_server.get_deltas() {
            match delta {
                dare::asset2::server::AssetServerDelta::HandleLoaded(handle) => {
                    if handle.is_type::<dare::asset2::assets::Buffer>() {
                        let asset_server = render_server.asset_server.clone();
                        let render_context = render_context.clone();
                        let err_handle = handle.clone();
                        let fut = rt.runtime.spawn(async move {
                                if let Some(metadata) = asset_server.get_metadata::<dare::asset2::assets::Buffer>(&handle) {
                                    let mut allocator = render_context.inner.allocator.clone();
                                    let transfer_pool = render_context.transfer_pool();
                                    let staging_size = transfer_pool.staging_size() as usize;

                                    Some(super::components::RenderBuffer::load_asset(
                                        metadata,
                                        super::components::BufferPrepareInfo {
                                            allocator,
                                            handle: dare::asset2::AssetHandleUntyped::from(handle).into_typed_handle::<
                                                crate::asset2::prelude::assets::Buffer
                                            >().unwrap(),
                                            transfer_pool,
                                            usage_flags: vk::BufferUsageFlags::STORAGE_BUFFER
                                            | vk::BufferUsageFlags::VERTEX_BUFFER
                                            | vk::BufferUsageFlags::INDEX_BUFFER
                                            | vk::BufferUsageFlags::TRANSFER_SRC
                                            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                                            location: MemoryLocation::GpuOnly,
                                        },
                                        dare::asset2::assets::BufferStreamInfo {
                                            chunk_size: staging_size,
                                        }
                                    ).await.map_err(move |e| {
                                        (e, err_handle)
                                    }))
                                } else {
                                    None
                                }
                            });
                        futures.push(fut.map_err(move |e| (e, err_handle)));
                    }
                }
                dare::asset2::server::AssetServerDelta::HandleUnloaded(_) => {
                    // Handle unload if necessary
                }
            }
        }
        // Process each future in the unordered set as it completes
        let render_asset_server = render_asset_server.clone();
        rt.runtime.spawn(async move {
            while let Some(result) = futures.next().await {
                match result {
                    Ok(Some(Ok(asset))) => {
                        // Handle the loaded asset here, such as adding it to storage
                        let handle = asset.handle.clone();
                        buffer_server.insert(asset.handle.clone().id(), asset);
                        unsafe {
                            render_asset_server
                                .asset_server
                                .update_state(
                                    &handle.into_untyped_handle(),
                                    dare::asset2::AssetState::Loaded,
                                )
                                .unwrap()
                        }
                    }
                    Ok(None) => {
                        // Handle the case where metadata was unavailable
                    }
                    Ok(Some(Err((e, handle)))) => {
                        tracing::error!("Error during streaming: {}", e);
                        unsafe {
                            render_asset_server
                                .asset_server
                                .update_state(&handle, dare::asset2::AssetState::Failed)
                                .unwrap()
                        }
                    }
                    Err((e, handle)) => {
                        // Log or handle any errors that occurred during the task execution
                        tracing::error!("Failed to load asset: {}", e);
                        unsafe {
                            render_asset_server
                                .asset_server
                                .update_state(&handle, dare::asset2::AssetState::Failed)
                                .unwrap()
                        }
                    }
                }
            }
        });
    }
    // handle any subsequent asset linking requests
    while let Ok(delta) = ir_recv.0.try_recv() {
        match delta {
            InnerRenderServerRequest::Delta(delta) => match delta {
                RenderServerAssetRelationDelta::Entry(entity, mesh) => {
                    let mesh = dare::engine::components::Mesh {
                        surface: mesh.surface.downgrade(),
                        bounding_box: mesh.bounding_box.clone(),
                        name: mesh.name.clone(),
                        transform: mesh.transform,
                    };
                    let entity_id = commands.spawn(mesh.clone());
                    let entity_id = entity_id.id();
                    meshes.0.entry(entity).or_insert(entity_id);
                }
                RenderServerAssetRelationDelta::Remove(_) => {}
            },
        }
    }
    // process of what is in queue
    buffers.process();
}
