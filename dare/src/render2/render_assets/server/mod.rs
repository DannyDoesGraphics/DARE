use crate::prelude as dare;
use crate::prelude::render::{InnerRenderServerRequest, RenderServerAssetRelationDelta};
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use crate::render2::server::IrRecv;
use bevy_ecs::prelude as becs;
use dagal::allocators::{GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use futures::stream::FuturesUnordered;
use futures::{StreamExt, TryFutureExt};
use std::hash::{Hash, Hasher};
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

const BUFFER_NAMES: [&'static str; 5] = [
    "Vertex Buffer",
    "Index Buffer",
    "Normal Buffer",
    "UV buffer",
    "Tangent buffer",
];

pub fn load_assets_to_gpu_in_world(
    render_context: becs::Res<dare::render::contexts::RenderContext>,
    query: becs::Query<
        (
            &dare::engine::components::Surface,
            Option<&dare::engine::components::Name>,
        ),
        becs::Added<dare::engine::components::Surface>,
    >,
    mut buffers: becs::ResMut<
        '_,
        super::assets::RenderAssetsStorage<super::components::RenderBuffer<GPUAllocatorImpl>>,
    >,
    render_asset_server: becs::ResMut<'_, RenderAssetServer>,
    rt: becs::Res<'_, dare::concurrent::BevyTokioRunTime>,
) {
    let mut futures = FuturesUnordered::new();
    let render_asset_server = render_asset_server.asset_server.clone();
    for (surface, name) in query.iter() {
        let buffers = [
            Some(surface.vertex_buffer.clone().into_untyped_handle()),
            Some(surface.index_buffer.clone().into_untyped_handle()),
            surface
                .normal_buffer
                .as_ref()
                .map(|normal_buffer| normal_buffer.clone().into_untyped_handle()),
            surface
                .uv_buffer
                .as_ref()
                .map(|uv_buffer| uv_buffer.clone().into_untyped_handle()),
            surface
                .tangent_buffer
                .as_ref()
                .map(|tangent_buffer| tangent_buffer.clone().into_untyped_handle()),
        ];
        for (index, handle) in buffers.into_iter().enumerate() {
            if let Some(handle) = handle {
                if let Some(metadata) =
                    render_asset_server.get_metadata::<dare::asset2::assets::Buffer>(&handle)
                {
                    let handle_state = render_asset_server.get_state(&handle);
                    if handle_state == Some(dare::asset2::AssetState::Unloaded) {
                        let err_handle = handle.clone();
                        let render_context = render_context.clone();
                        let name: Option<String> = BUFFER_NAMES.get(index).map(|v| {
                            format!(
                                "{} {}",
                                name.map(|name| name.0.as_str()).unwrap_or({
                                    let mut hasher = std::hash::DefaultHasher::default();
                                    surface.hash(&mut hasher);
                                    hasher.finish().to_string().as_str()
                                }),
                                v
                            )
                        });
                        let fut = rt.runtime.spawn(async move {
                            let mut allocator = render_context.inner.allocator.clone();
                            let transfer_pool = render_context.transfer_pool();
                            let staging_size = transfer_pool.gpu_staging_size() as usize;
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
                                    name,
                                },
                                dare::asset2::assets::BufferStreamInfo {
                                    chunk_size: staging_size,
                                }
                            ).await.map_err(move |e| {
                                (e, err_handle)
                            }))
                        });
                        futures.push(fut);
                    } else if handle_state != Some(dare::asset2::AssetState::Loaded) {
                        tracing::warn!("Asset is in unexpected state: {:?}", handle_state);
                    }
                } else {
                    tracing::error!("No metadata found for {:?}", handle);
                }
            }
        }
    }

    let buffer_server = buffers.server();
    // now handle putting the loaded assets in their place after
    rt.runtime.spawn(async move {
        while let Some(result) = futures.next().await {
            match result {
                Ok(Some(Ok(asset))) => {
                    // Handle the loaded asset here, such as adding it to storage
                    let handle = asset.handle.clone();
                    buffer_server.insert(asset.handle.clone().id(), asset);
                    unsafe {
                        render_asset_server
                            .update_state(
                                &handle.into_untyped_handle(),
                                dare::asset2::AssetState::Loaded,
                            )
                            .unwrap()
                    }
                }
                Ok(Some(Err((e, handle)))) => {
                    tracing::error!("Error during streaming with handle {:?}: {:?}", handle, e);
                    unsafe {
                        render_asset_server
                            .update_state(&handle, dare::asset2::AssetState::Failed)
                            .unwrap()
                    }
                }
                Ok(None) => {
                    // Handle the case where metadata was unavailable
                    tracing::error!("Cannot find metadata to load surface");
                }
                Err(e) => {
                    tracing::error!("Error during streaming: {}", e);
                }
            }
        }
    });
}

/// Process incoming packets from engine server indicating the relations between each asset
pub fn process_asset_relations_incoming_system(
    mut commands: becs::Commands,
    mut buffers: becs::ResMut<
        '_,
        super::assets::RenderAssetsStorage<super::components::RenderBuffer<GPUAllocatorImpl>>,
    >,
    mut meshes: becs::ResMut<dare::render::resource_relationship::Meshes>,
    ir_recv: becs::ResMut<'_, IrRecv>,
) {
    // handle any subsequent asset linking requests
    while let Ok(delta) = ir_recv.0.try_recv() {
        match delta {
            InnerRenderServerRequest::Delta(delta) => match delta {
                RenderServerAssetRelationDelta::Entry(entity, mesh) => {
                    if !meshes.0.contains_key(&entity) {
                        let mesh = dare::engine::components::Mesh {
                            surface: mesh.surface.downgrade(),
                            bounding_box: mesh.bounding_box.clone(),
                            name: mesh.name.clone(),
                            transform: mesh.transform,
                        };
                        let entity_id = commands.spawn(mesh.clone());
                        let entity_id = entity_id.id();
                        meshes.0.insert(entity, entity_id);
                    }
                }
                RenderServerAssetRelationDelta::Remove(_) => {}
            },
        }
    }
    // process of what is in queue
    buffers.process();
}
