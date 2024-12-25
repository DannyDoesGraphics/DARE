pub mod info;
pub mod linker;

use std::any::TypeId;
pub use info::*;
pub use linker::*;

use crate::prelude as dare;
use crate::prelude::render::{InnerRenderServerRequest, RenderServerAssetRelationDelta};
use crate::render2::render_assets::traits::MetaDataRenderAsset;
use crate::render2::server::IrRecv;
use bevy_ecs::prelude as becs;
use bevy_ecs::query::Has;
use dagal::allocators::{GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use futures::stream::FuturesUnordered;
use futures::{StreamExt, TryFutureExt};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use dare_containers as containers;

/// System is responsible for taking in deltas of the asset server and updating every
/// [`RenderAssetManagerStorage`]
pub fn asset_manager_system(
    asset_server: becs::Res<dare::asset2::server::AssetServer>
) {
    match asset_server.flush() {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Failed to flush asset server due to {e}");
        }
    }
    for delta in asset_server.get_deltas() {
        match delta {
            dare::asset2::server::AssetServerDelta::HandleLoaded(loaded) => {}
            dare::asset2::server::AssetServerDelta::HandleUnloaded(_) => {}
        }
    }
}


/// Works as a link between the [`dare::asset2::server::AssetServer`] and the render manager
#[derive(Default, Debug, becs::Resource)]
pub struct MeshLink {
    link: bevy_ecs::entity::EntityHashMap<dare::engine::components::Mesh>,
}

impl Deref for MeshLink {
    type Target = bevy_ecs::entity::EntityHashMap<dare::engine::components::Mesh>;

    fn deref(&self) -> &Self::Target {
        &self.link
    }
}

impl DerefMut for MeshLink {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.link
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
    render_context: &dare::render::contexts::RenderContext,
    surfaces: &[(
        dare::engine::components::Surface,
        Option<dare::engine::components::Name>,
    )],
    buffer_server: super::assets::RenderAssets<
        dare::render::render_assets::components::RenderBuffer<GPUAllocatorImpl>,
    >,
    asset_server: dare::asset2::server::AssetServer,
    rt: &dare::concurrent::BevyTokioRunTime,
) {
    let mut futures = FuturesUnordered::new();
    for (surface, name) in surfaces.iter() {
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
                    asset_server.get_metadata::<dare::asset2::assets::Buffer>(&handle)
                {
                    let handle_state = asset_server.get_state(&handle);
                    if handle_state == Some(dare::asset2::AssetState::Unloaded) {
                        let err_handle = handle.clone();
                        let succ_handle = handle.clone();
                        let render_context = render_context.clone();
                        let name: Option<String> = BUFFER_NAMES.get(index).map(|v| {
                            format!(
                                "{} {}",
                                name.as_ref().map(|name| name.0.as_str()).unwrap_or({
                                    let mut hasher = std::hash::DefaultHasher::default();
                                    surface.hash(&mut hasher);
                                    hasher.finish().to_string().as_str()
                                }),
                                v
                            )
                        });
                        unsafe {
                            asset_server
                                .update_state(&*handle, dare::asset2::AssetState::Loading)
                                .unwrap();
                        }
                        let render_asset_server = asset_server.clone();
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
                                    tracing::error!("Failed to load asset: {e}");
                                    (e, err_handle)
                                })).map(|r| {
                                    unsafe {
                                        render_asset_server.update_state(
                                            &succ_handle, dare::asset2::AssetState::Loaded
                                        ).unwrap();
                                    }
                                    r
                                })
                            });
                        futures.push(fut);
                    } else if handle_state != Some(dare::asset2::AssetState::Loaded) {
                        tracing::warn!(
                            "Asset is in unexpected state: {:?}, ignoring",
                            handle_state
                        );
                    }
                } else {
                    tracing::error!("No metadata found for {:?}", handle);
                }
            }
        }
    }

    // now handle putting the loaded assets in their place after
    rt.runtime.spawn(async move {
        while let Some(result) = futures.next().await {
            match result {
                Ok(Some(Ok(asset))) => {
                    // Handle the loaded asset here, such as adding it to storage
                    let handle = asset.handle.clone();
                    buffer_server.insert(asset.handle.clone().id(), asset);
                    unsafe {
                        asset_server
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
                        asset_server
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

/// Process incoming packets from engine manager indicating the relations between each asset
pub fn process_asset_relations_incoming_system(
    render_context: becs::Res<'_, dare::render::contexts::RenderContext>,
    mut buffers: becs::ResMut<
        '_,
        super::assets::RenderAssetsStorage<super::components::RenderBuffer<GPUAllocatorImpl>>,
    >,
    mut asset_server: becs::ResMut<'_, dare::asset2::server::AssetServer>,
    mut mesh_link: becs::ResMut<'_, MeshLink>,
    rt: becs::Res<'_, dare::concurrent::BevyTokioRunTime>,
    ir_recv: becs::ResMut<'_, IrRecv>,
) {
    let mut added_surfaces: Vec<(
        dare::engine::components::Surface,
        Option<dare::engine::components::Name>,
    )> = Vec::new();
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
                    if let Some(replaced_mesh) = mesh_link.insert(entity, mesh) {
                        tracing::warn!("Unexpected mesh replacement: {:?}", replaced_mesh);
                    }
                }
                RenderServerAssetRelationDelta::Remove(entity) => {
                    if mesh_link.remove(&entity).is_none() {
                        tracing::warn!("Tried to remove mesh at entity {entity}, got None.");
                    }
                }
            },
        }
    }
    // process of what is in queue
    load_assets_to_gpu_in_world(
        &render_context,
        &added_surfaces,
        buffers.server(),
        asset_server.clone(),
        &rt,
    );
    buffers.process();
}
