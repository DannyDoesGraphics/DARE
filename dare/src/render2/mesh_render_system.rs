use crate::asset2::assets::BufferStreamInfo;
use crate::asset2::prelude::AssetHandle;
use crate::prelude as dare;
use crate::render2::c::CPushConstant;
use crate::render2::physical_resource;
use crate::render2::physical_resource::{BufferPrepareInfo, VirtualResource};
use crate::render2::prelude::util::TransferPool;
use bevy_ecs::prelude::*;
use dagal::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use dagal::ash::vk::Handle;
use dagal::command::CommandBufferState;
use dagal::command::command_buffer::CmdBuffer;
use dagal::pipelines::Pipeline;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use std::collections::{HashMap, HashSet};
use std::mem::size_of;

/// Functions effectively as a collection
struct SurfaceRender<'a> {
    surface: &'a dare::engine::components::Surface,
    transform: &'a dare::physics::components::Transform,
}
impl<'a> SurfaceRender<'a> {
    pub fn decompose(
        self,
    ) -> (
        &'a dare::engine::components::Surface,
        &'a dare::physics::components::Transform,
    ) {
        (self.surface, self.transform)
    }
}

pub fn build_instancing_data(
    view_proj: glam::Mat4,
    query: &Query<
        '_,
        '_,
        (
            Entity,
            &dare::engine::components::Surface,
            Option<&dare::engine::components::Material>,
            &dare::render::components::BoundingBox,
            &dare::physics::components::Transform,
            &dare::engine::components::Name,
        ),
    >,
    allocator: ArcAllocator<GPUAllocatorImpl>,
    transfer_pool: TransferPool<GPUAllocatorImpl>,
    textures: &mut physical_resource::PhysicalResourceStorage<
        physical_resource::RenderImage<GPUAllocatorImpl>,
    >,
    samplers: &mut physical_resource::PhysicalResourceStorage<dare::asset2::assets::SamplerAsset>,
    buffers: &mut physical_resource::PhysicalResourceStorage<
        physical_resource::RenderBuffer<GPUAllocatorImpl>,
    >,
) -> (
    Vec<dare::engine::components::Surface>,
    Vec<dare::render::c::CSurface>,
    Vec<dare::render::c::CMaterial>,
    Vec<dare::render::c::InstancedSurfacesInfo>,
    Vec<[f32; 16]>,
    HashSet<VirtualResource>,
) {
    let build_instancing_data_span = tracy_client::span!("build_instancing_data_span");
    build_instancing_data_span.emit_value(query.iter().len() as u64);
    // Acquire a tightly packed map
    let mut surface_map: HashMap<dare::engine::components::Surface, Option<usize>> =
        HashMap::with_capacity(query.iter().len());
    let mut unique_surfaces: Vec<dare::render::c::CSurface> = Vec::new();
    let mut asset_unique_surfaces: Vec<dare::engine::components::Surface> = Vec::new();
    // Collection of strong virtual handles
    let mut used_resources: HashSet<VirtualResource> = HashSet::new();

    let mut material_map: HashMap<dare::engine::components::Material, usize> =
        HashMap::with_capacity(surface_map.len());
    let mut unique_materials: Vec<dare::render::c::CMaterial> = vec![dare::render::c::CMaterial {
        bit_flag: 0,
        _padding: 0,
        color_factor: glam::Vec4::ONE.to_array(),
        albedo_texture_id: 0,
        albedo_sampler_id: 0,
        normal_texture_id: 0,
        normal_sampler_id: 0,
    }];
    for (_index, (_entity, surface, material, bounding_box, transform, name)) in
        query.iter().enumerate()
    {
        // check if it even exists in frame
        if !bounding_box.visible_in_frustum(transform.get_transform_matrix(), view_proj) {
            continue;
        }
        surface_map.entry((*surface).clone()).or_insert_with(|| {
            let id: usize = unique_surfaces.len();
            // attempt a load of everything
            //println!("Index: {:?}", &surface.index_buffer);
            const BUFFER_LIFETIME: u32 = 64;
            buffers.load_or_create_asset_handle(
                surface.index_buffer.clone(),
                BufferPrepareInfo {
                    allocator: allocator.clone(),
                    handle: surface.index_buffer.clone(),
                    transfer_pool: transfer_pool.clone(),
                    usage_flags: vk::BufferUsageFlags::TRANSFER_SRC
                        | vk::BufferUsageFlags::TRANSFER_DST
                        | vk::BufferUsageFlags::INDEX_BUFFER
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                    location: MemoryLocation::GpuOnly,
                    name: Some(format!("{}_index_buffer", name.0.clone())),
                },
                BufferStreamInfo {
                    chunk_size: transfer_pool
                        .cpu_staging_size()
                        .min(transfer_pool.gpu_staging_size())
                        as usize,
                },
                BUFFER_LIFETIME,
            );
            //println!("Vertex: {:?}", &surface.vertex_buffer.id());
            buffers.load_or_create_asset_handle(
                surface.vertex_buffer.clone(),
                BufferPrepareInfo {
                    allocator: allocator.clone(),
                    handle: surface.vertex_buffer.clone(),
                    transfer_pool: transfer_pool.clone(),
                    usage_flags: vk::BufferUsageFlags::TRANSFER_SRC
                        | vk::BufferUsageFlags::TRANSFER_DST
                        | vk::BufferUsageFlags::VERTEX_BUFFER
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                    location: MemoryLocation::GpuOnly,
                    name: Some(format!("{}_vertex_buffer", name.0.clone())),
                },
                BufferStreamInfo {
                    chunk_size: transfer_pool
                        .cpu_staging_size()
                        .min(transfer_pool.gpu_staging_size())
                        as usize,
                },
                BUFFER_LIFETIME,
            );
            //println!("Normal: {:?}", &surface.normal_buffer);
            if let Some(buffer) = surface.normal_buffer.as_ref() {
                buffers.load_or_create_asset_handle(
                    buffer.clone(),
                    BufferPrepareInfo {
                        allocator: allocator.clone(),
                        handle: buffer.clone(),
                        transfer_pool: transfer_pool.clone(),
                        usage_flags: vk::BufferUsageFlags::TRANSFER_SRC
                            | vk::BufferUsageFlags::TRANSFER_DST
                            | vk::BufferUsageFlags::STORAGE_BUFFER
                            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                        location: MemoryLocation::GpuOnly,
                        name: Some(format!("{}_normal_buffer", name.0.clone())),
                    },
                    BufferStreamInfo {
                        chunk_size: transfer_pool
                            .cpu_staging_size()
                            .min(transfer_pool.gpu_staging_size())
                            as usize,
                    },
                    BUFFER_LIFETIME,
                );
            }
            //println!("UV: {:?}", &surface.uv_buffer);
            if let Some(buffer) = surface.uv_buffer.as_ref() {
                buffers.load_or_create_asset_handle(
                    buffer.clone(),
                    BufferPrepareInfo {
                        allocator: allocator.clone(),
                        handle: buffer.clone(),
                        transfer_pool: transfer_pool.clone(),
                        usage_flags: vk::BufferUsageFlags::TRANSFER_SRC
                            | vk::BufferUsageFlags::TRANSFER_DST
                            | vk::BufferUsageFlags::STORAGE_BUFFER
                            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                        location: MemoryLocation::GpuOnly,
                        name: Some(format!("{}_uv_buffer", name.0.clone())),
                    },
                    BufferStreamInfo {
                        chunk_size: transfer_pool
                            .cpu_staging_size()
                            .min(transfer_pool.gpu_staging_size())
                            as usize,
                    },
                    BUFFER_LIFETIME,
                );
            }
            if let Some(buffer) = surface.tangent_buffer.as_ref() {
                buffers.load_or_create_asset_handle(
                    buffer.clone(),
                    BufferPrepareInfo {
                        allocator: allocator.clone(),
                        handle: buffer.clone(),
                        transfer_pool: transfer_pool.clone(),
                        usage_flags: vk::BufferUsageFlags::TRANSFER_SRC
                            | vk::BufferUsageFlags::TRANSFER_DST
                            | vk::BufferUsageFlags::STORAGE_BUFFER
                            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                        location: MemoryLocation::GpuOnly,
                        name: Some(format!("{}_tangent_buffer", name.0.clone())),
                    },
                    BufferStreamInfo {
                        chunk_size: transfer_pool
                            .cpu_staging_size()
                            .min(transfer_pool.gpu_staging_size())
                            as usize,
                    },
                    BUFFER_LIFETIME,
                );
            }

            let mut buffer_resolve =
                |virtual_resource: &AssetHandle<dare::asset2::assets::Buffer>| {
                    // retrieve virtual resource, if we can, resolve for physical
                    buffers
                        .resolve_virtual_resource(virtual_resource)
                        .map(|vr| vr.upgrade())
                        .flatten()
                        .and_then(|vr| {
                            buffers.resolve(&vr).map(|_| {
                                used_resources.insert(vr.clone());
                                vr
                            })
                        })
                };

            // Check if the required buffers can be resolved and upgrade them
            buffer_resolve(&surface.index_buffer)?;
            buffer_resolve(&surface.vertex_buffer)?;
            if let Some(buffer) = surface.uv_buffer.as_ref() {
                buffer_resolve(buffer)?;
            }
            if let Some(buffer) = surface.tangent_buffer.as_ref() {
                buffer_resolve(buffer)?;
            }

            if let Some(c_surface) =
                dare::render::c::CSurface::from_surface(buffers, (*surface).clone())
            {
                unique_surfaces.push(c_surface);
                asset_unique_surfaces.push((*surface).clone());
                Some(id)
            } else {
                None
            }
        });
        material_map
            .entry(material.cloned().unwrap_or({
                dare::engine::components::Material {
                    albedo_factor: glam::Vec4::ONE,
                    albedo_texture: None,
                    alpha_mode: gltf::material::AlphaMode::Opaque,
                }
            }))
            .or_insert_with(|| {
                let id: usize = unique_materials.len();
                if let Some(material) = material.cloned() {
                    material.albedo_texture.as_ref().map(|texture| {
                        /*
                        textures.load_or_create_asset_handle(
                            texture.asset_handle.clone(),
                            (
                                allocator.clone(),
                                texture.asset_handle.clone(),
                                transfer_pool.clone(),
                                Some(format!("{}_albedo_texture", name.0.clone())),
                            ),
                            (),
                            128,
                        );*/
                        /*
                        samplers.load_or_create(
                            texture.sampler.clone(),
                            (),
                            (),
                            128
                        );
                        */
                    });

                    match dare::render::c::CMaterial::from_material(
                        allocator.clone(),
                        transfer_pool.clone(),
                        textures,
                        material,
                    ) {
                        None => 0,
                        Some(material) => {
                            unique_materials.push(material);
                            id
                        }
                    }
                } else {
                    0
                }
            });
    }

    /// (surface_index, material_index) -> transforms
    let mut instance_groups: HashMap<(u64, u64), Vec<glam::Mat4>> = HashMap::new();
    for (_index, (_entity, surface, material, _bounding_box, transform, name)) in
        query.iter().enumerate()
    {
        // ignore surfaces which failed to resolve
        if surface_map
            .get(surface)
            .map(|idx| idx.is_none())
            .unwrap_or(true)
        {
            continue;
        }

        // focus on grouping for instancing
        instance_groups
            .entry((
                surface_map.get(surface).unwrap().unwrap() as u64,
                // default to 0 for the default material
                material
                    .map(|material| *material_map.get(material).unwrap() as u64)
                    .unwrap_or(0),
            ))
            .or_default()
            .push(transform.get_transform_matrix());
    }

    // turn all transformations into one global buffer
    let mut instancing_information: Vec<dare::render::c::InstancedSurfacesInfo> =
        Vec::with_capacity(instance_groups.len());
    let mut transforms: Vec<[f32; 16]> = Vec::new();
    for ((surface, material), transformations) in instance_groups.iter() {
        instancing_information.push(dare::render::c::InstancedSurfacesInfo {
            surface: *surface,
            material: *material,
            instances: transformations.len() as u64,
            transformation_offset: transforms.len() as u64,
        });
        transforms.append(
            &mut transformations
                .iter()
                .map(|transform| transform.transpose().to_cols_array())
                .collect::<Vec<[f32; 16]>>(),
        );
    }
    drop(build_instancing_data_span);
    (
        asset_unique_surfaces,
        unique_surfaces,
        unique_materials,
        instancing_information,
        transforms,
        used_resources,
    )
}

pub async fn mesh_render(
    frame_number: usize,
    render_context: super::render_context::RenderContext,
    camera: &dare::render::components::camera::Camera,
    frame: &mut super::frame::Frame,
    surfaces: Query<
        '_,
        '_,
        (
            Entity,
            &dare::engine::components::Surface,
            Option<&dare::engine::components::Material>,
            &dare::render::components::BoundingBox,
            &dare::physics::components::Transform,
            &dare::engine::components::Name,
        ),
    >,
    mut textures: &mut physical_resource::PhysicalResourceStorage<
        physical_resource::RenderImage<GPUAllocatorImpl>,
    >,
    samplers: &mut physical_resource::PhysicalResourceStorage<dare::asset2::assets::SamplerAsset>,
    mut buffers: &mut physical_resource::PhysicalResourceStorage<
        physical_resource::RenderBuffer<GPUAllocatorImpl>,
    >,
) {
    let mesh_render_span = tracy_client::span!("mesh_render");
    #[cfg(feature = "tracing")]
    tracing::trace!("Rendering meshes into {frame_number}");
    {
        match &frame.command_buffer {
            CommandBufferState::Ready(_) => {
                panic!("Mesh recording invalid cmd buffer state")
            }
            CommandBufferState::Recording(recording) => {
                // Culling step
                let (
                    asset_surfaces,
                    surfaces,
                    materials,
                    instancing_information,
                    transforms,
                    used_virtual_resources,
                ) = {
                    let view_proj = camera.get_projection(
                        frame.image_extent.width as f32 / frame.image_extent.height as f32,
                    ) * camera.get_view_matrix();
                    build_instancing_data(
                        view_proj,
                        &surfaces,
                        render_context.inner.allocator.clone(),
                        render_context.transfer_pool(),
                        textures,
                        samplers,
                        buffers,
                    )
                };
                frame.resources.clear();
                frame.resources = used_virtual_resources;
                // check for empty surfaces, before going
                if instancing_information.is_empty() {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("No instances found, skipping render.");
                    return;
                }

                // generate indirect calls
                let indirect_calls: Vec<vk::DrawIndexedIndirectCommand> = instancing_information
                    .iter()
                    .map(|instancing| vk::DrawIndexedIndirectCommand {
                        index_count: asset_surfaces[instancing.surface as usize].index_count as u32,
                        instance_count: instancing.instances as u32,
                        first_index: 0,
                        vertex_offset: 0,
                        first_instance: 0,
                    })
                    .collect();
                let mut instanced_surfaces_bytes_offset: Vec<u64> = vec![0];
                // upload indirect calls
                frame
                    .indirect_buffer
                    .upload_to_buffer(
                        &render_context.inner.immediate_submit,
                        indirect_calls.as_slice(),
                    )
                    .await
                    .unwrap();
                // upload instanced information
                frame
                    .instanced_buffer
                    .upload_to_buffer(
                        &render_context.inner.immediate_submit,
                        instancing_information
                            .iter()
                            .flat_map(|instancing| {
                                let bytes = bytemuck::bytes_of(instancing).to_vec();
                                instanced_surfaces_bytes_offset.push(
                                    instanced_surfaces_bytes_offset.last().unwrap()
                                        + bytes.len() as u64,
                                );
                                bytes
                            })
                            .collect::<Vec<u8>>()
                            .as_slice(),
                    )
                    .await
                    .unwrap();

                // upload surface information
                let surface_slice = surfaces
                    .iter()
                    .flat_map(bytemuck::bytes_of)
                    .copied()
                    .collect::<Vec<u8>>();
                let material_slice = materials
                    .iter()
                    .flat_map(bytemuck::bytes_of)
                    .copied()
                    .collect::<Vec<u8>>();
                let transform_slice = transforms
                    .iter()
                    .flat_map(bytemuck::bytes_of)
                    .copied()
                    .collect::<Vec<u8>>();
                tokio::try_join!(
                    frame.surface_buffer.upload_to_buffer(
                        &render_context.inner.immediate_submit,
                        surface_slice.as_slice(),
                    ),
                    frame.material_buffer.upload_to_buffer(
                        &render_context.inner.immediate_submit,
                        material_slice.as_slice(),
                    ),
                    frame.transform_buffer.upload_to_buffer(
                        &render_context.inner.immediate_submit,
                        transform_slice.as_slice(),
                    )
                )
                .unwrap();
                // begin rendering
                let dynamic_rendering = unsafe {
                    recording
                        .dynamic_rendering()
                        .push_image_as_color_attachment(
                            vk::ImageLayout::GENERAL,
                            &frame.draw_image_view,
                            None,
                        )
                        .depth_attachment_info(
                            *frame.depth_image_view.as_raw(),
                            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                        )
                        .begin_rendering(vk::Extent2D {
                            width: frame.image_extent.width,
                            height: frame.image_extent.height,
                        })
                };
                let viewport = vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: frame.draw_image.extent().width as f32,
                    height: frame.draw_image.extent().height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                };
                unsafe {
                    render_context.inner.device.get_handle().cmd_set_viewport(
                        recording.handle(),
                        0,
                        &[viewport],
                    );
                }
                let scissor = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: frame.draw_image.extent().width,
                        height: frame.draw_image.extent().height,
                    },
                };

                unsafe {
                    render_context.inner.device.get_handle().cmd_set_scissor(
                        recording.handle(),
                        0,
                        &[scissor],
                    );
                }

                // bind pipeline
                unsafe {
                    render_context.inner.device.get_handle().cmd_bind_pipeline(
                        recording.handle(),
                        vk::PipelineBindPoint::GRAPHICS,
                        render_context.inner.graphics_pipeline.handle(),
                    );
                }
                let view_proj = {
                    let camera_view = camera.get_view_matrix();
                    let camera_proj = camera.get_projection(
                        frame.image_extent.width as f32 / frame.image_extent.height as f32,
                    );
                    let view_proj = camera_proj * camera_view;
                    view_proj
                };

                let mut push_constant = CPushConstant {
                    transform: view_proj.to_cols_array(),
                    instanced_surface_info: frame.instanced_buffer.get_buffer().address(),
                    surface_infos: frame.surface_buffer.get_buffer().address(),
                    transforms: frame.transform_buffer.get_buffer().address(),
                    draw_id: 0,
                };
                for (index, instancing) in instancing_information.iter().enumerate() {
                    let surface_asset = &asset_surfaces[instancing.surface as usize];
                    let index_buffer = buffers
                        .resolve_asset(&asset_surfaces[instancing.surface as usize].index_buffer)
                        .unwrap();
                    // push new constants
                    push_constant.instanced_surface_info =
                        frame.instanced_buffer.get_buffer().address()
                            + instanced_surfaces_bytes_offset[index] as vk::DeviceAddress;
                    //println!("Instanced surface offset: {:?} or {:?} or {:?}", instanced_surfaces_bytes_offset[index], instancing_information[index], surfaces[instancing_information[index].surface as usize]);
                    let draw_id: u32 = (surfaces[instancing.surface as usize].positions
                        % u32::MAX as u64)
                        .try_into()
                        .unwrap();
                    push_constant.draw_id = draw_id as u64;
                    unsafe {
                        let bytes: &[u8] = std::slice::from_raw_parts(
                            &push_constant as *const CPushConstant as *const u8,
                            size_of::<CPushConstant>(),
                        );
                        render_context.inner.device.get_handle().cmd_push_constants(
                            recording.handle(),
                            *render_context.inner.graphics_layout.as_raw(),
                            vk::ShaderStageFlags::VERTEX,
                            0,
                            bytes,
                        );
                    }

                    // indirect draw
                    unsafe {
                        render_context
                            .inner
                            .device
                            .get_handle()
                            .cmd_bind_index_buffer(
                                recording.handle(),
                                *index_buffer.as_raw(),
                                0,
                                vk::IndexType::UINT32,
                            );
                        render_context
                            .inner
                            .device
                            .get_handle()
                            .cmd_draw_indexed_indirect(
                                recording.handle(),
                                *frame.indirect_buffer.get_buffer().as_raw(),
                                (index * size_of::<vk::DrawIndexedIndirectCommand>())
                                    as vk::DeviceSize,
                                1,
                                size_of::<vk::DrawIndexedIndirectCommand>() as u32,
                            );
                    }
                }
                dynamic_rendering.end_rendering();
            }
            CommandBufferState::Executable(_) => {
                panic!("Mesh recording invalid cmd buffer state")
            }
        };
    }
    drop(mesh_render_span);
}
