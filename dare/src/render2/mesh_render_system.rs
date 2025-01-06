use crate::prelude as dare;
use crate::prelude::render::util::GPUResourceTable;
use crate::render2::c::CPushConstant;
use bevy_ecs::prelude::*;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::ash::vk::Handle;
use dagal::command::command_buffer::CmdBuffer;
use dagal::command::CommandBufferState;
use dagal::pipelines::Pipeline;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use image::imageops::unsharpen;
use tokio::task;
use crate::render2::render_assets::storage::asset_manager_system;

/// Functions effectively as a collection
struct SurfaceRender<'a> {
    surface: &'a dare::engine::components::Surface,
    transform: &'a dare::physics::components::Transform,
}
impl<'a> SurfaceRender<'a> {
    pub fn decompose(self) -> (&'a dare::engine::components::Surface, &'a dare::physics::components::Transform) {
        (self.surface, self.transform)
    }
}

pub fn build_instancing_data(
    surfaces: &[&dare::engine::components::Surface],
    materials: &[Option<&dare::engine::components::Material>],
    transformations: &[glam::Mat4],
    buffers: &dare::render::render_assets::storage::RenderAssetManagerStorage<
        dare::render::render_assets::components::RenderBuffer<GPUAllocatorImpl>
    >
) -> (
    Vec<dare::engine::components::Surface>,
    Vec<dare::render::c::CSurface>,
    Vec<dare::render::c::CMaterial>,
    Vec<dare::render::c::InstancedSurfacesInfo>,
    Vec<[f32; 16]>
) {
    assert_eq!(surfaces.len(), materials.len());
    assert_eq!(transformations.len(), surfaces.len());

    /// Acquire a tightly packed map
    let mut unique_surfaces: Vec<dare::render::c::CSurface> = Vec::new();
    let mut asset_unique_surfaces: Vec<dare::engine::components::Surface> = Vec::new();
    let mut surface_map: HashMap<dare::engine::components::Surface, Option<usize>> = HashMap::with_capacity(surfaces.len());
    for (index, surface) in surfaces.iter().enumerate() {
        println!("{:?} -> {:?}", surface.index_count, transformations[index].w_axis.x);
        surface_map.entry((*surface).clone()).or_insert_with(|| {
            let id: usize = unique_surfaces.len();
            if let Some(c_surface) = dare::render::c::CSurface::from_surface(buffers, (*surface).clone()) {
                unique_surfaces.push(c_surface);
                asset_unique_surfaces.push((*surface).clone());
                Some(id)
            } else {
                None
            }
        });
    }

    let mut unique_materials: Vec<dare::render::c::CMaterial> = Vec::new();
    let mut material_map: HashMap<dare::engine::components::Material, usize> = HashMap::with_capacity(materials.len());
    for (index, material) in materials.iter().enumerate() {
        // If surface could not resolve, we skip
        if surface_map.get(surfaces[index]).map(|index| index.is_none()).unwrap_or(true) {
            continue;
        }
        // Material default defined here
        material_map.entry((*material).cloned().unwrap_or({
            dare::engine::components::Material {
                albedo_factor: glam::Vec4::ONE,
            }
        })).or_insert_with(|| {
            let id: usize = unique_materials.len();
            if let Some(material) = (*material).cloned() {
                match dare::render::c::CMaterial::from_material(material) {
                    None => {
                        0
                    }
                    Some(material) => {
                        unique_materials.push(material);
                        id
                    }
                }
            } else {
                // No default material exists, make one
                unique_materials.push(dare::render::c::CMaterial {
                    bit_flag: 0,
                    _padding: 0,
                    color_factor: glam::Vec4::ONE.to_array(),
                    albedo_texture_id: 0,
                    albedo_sampler_id: 0,
                    normal_texture_id: 0,
                    normal_sampler_id: 0,
                });
                id
            }
        });
    }

    /// (surface_index, material_index) -> transforms
    let mut instance_groups: HashMap<(u64, u64), Vec<glam::Mat4>> = HashMap::new();
    for ((surface, material), transformation) in surfaces.iter().zip(materials.iter()).zip(transformations.iter()) {
        // ignore surfaces which failed to resolve
        if surface_map.get(*surface).map(|idx| idx.is_none()).unwrap_or(true) {
            continue;
        }

        // focus on grouping for instancing
        instance_groups.entry((
            surface_map.get(surface).unwrap().unwrap() as u64,
            // default to 0 for the default material
            material.map(|material| *material_map.get(material).unwrap() as u64).unwrap_or(0),
            )).or_insert_with(Vec::new)
            .push(transformation.clone());
    }

    // turn all transformations into one global buffer
    let mut instancing_information: Vec<dare::render::c::InstancedSurfacesInfo> = Vec::with_capacity(instance_groups.len());
    let mut transforms: Vec<[f32; 16]> = Vec::new();
    for ((surface, material), transformations) in instance_groups.iter() {
        instancing_information.push(dare::render::c::InstancedSurfacesInfo {
            surface: *surface,
            material: *material,
            instances: transformations.len() as u64,
            transformation_offset: transforms.len() as u64,
        });
        transforms.append(&mut transformations.iter().map(|transform| transform.transpose().to_cols_array()).collect::<Vec<[f32; 16]>>());
    }
    // sanity check
    for (instancing, (_, tfs)) in instancing_information.iter().zip(instance_groups.iter()) {
        let start = instancing.transformation_offset as usize;
        let end = instancing.transformation_offset as usize + instancing.instances as usize;
        if transforms[start..end]
            != tfs.iter().map(|t| t.transpose().to_cols_array()).collect::<Vec<[f32; 16]>>() {
            panic!("Not equivalent?");
        }
    }
    instancing_information.sort_by(|a, b| {
        asset_unique_surfaces[a.surface as usize].cmp(&asset_unique_surfaces[b.surface as usize])
    });

    (
        asset_unique_surfaces,
        unique_surfaces,
        unique_materials,
        instancing_information,
        transforms
    )
}

pub async fn mesh_render(
    frame_number: usize,
    render_context: super::render_context::RenderContext,
    camera: &dare::render::components::camera::Camera,
    frame: &mut super::frame::Frame,
    surfaces: Query<'_, '_, (Entity, &dare::engine::components::Surface, &dare::render::components::BoundingBox, &dare::physics::components::Transform)>,
    buffers: Res<
        '_,
        dare::render::render_assets::storage::RenderAssetManagerStorage<
            dare::render::render_assets::components::RenderBuffer<GPUAllocatorImpl>
        >
    >,
) {
    #[cfg(feature = "tracing")]
    tracing::trace!("Rendering meshes into {frame_number}");
    {
        let cmd_recording = match &frame.command_buffer {
            CommandBufferState::Ready(_) => {
                panic!("Mesh recording invalid cmd buffer state")
            }
            CommandBufferState::Recording(recording) => {
                let (asset_surfaces, surfaces, materials, instancing_information, transforms) = {
                    let mut sfs: Vec<&dare::engine::components::Surface> = Vec::new();
                    let mut materials: Vec<Option<&dare::engine::components::Material>> = Vec::new();
                    let mut transforms: Vec<glam::Mat4> = Vec::new();
                    for (_, surface, _, transform) in surfaces.iter() {
                        sfs.push(
                            surface
                        );
                        materials.push(None);
                        transforms.push(
                            transform.get_transform_matrix()
                        );
                    }
                    build_instancing_data(
                        &sfs,
                        &materials,
                        &transforms,
                        &buffers
                    )
                };
                // check for empty surfaces, before going
                if instancing_information.is_empty() {
                    return;
                }

                // generate indirect calls
                let indirect_calls: Vec<vk::DrawIndexedIndirectCommand> = instancing_information
                    .iter()
                    .map(|instancing| vk::DrawIndexedIndirectCommand {
                        index_count: asset_surfaces[instancing.surface as usize].index_count as u32,
                        instance_count: transforms[instancing.surface as usize].len() as u32,
                        first_index: 0,
                        vertex_offset: 0,
                        first_instance: 0,
                    })
                    .collect();
                // TODO: save handles for lifetime purposes
                // we only need the instanced info
                let mut instanced_surfaces_bytes_offset: Vec<u64> = vec![0];
                // upload indirect calls
                frame
                    .indirect_buffer
                    .upload_to_buffer(
                        &render_context.inner.immediate_submit,
                        indirect_calls.as_slice(),
                        render_context.inner.window_context.present_queue.get_family_index(),
                    )
                    .await
                    .unwrap();
                // upload instanced information
                frame
                    .instanced_buffer
                    .upload_to_buffer(
                        &render_context.inner.immediate_submit,
                        instancing_information.iter().flat_map(|instancing| {
                            let bytes = bytemuck::bytes_of(instancing).to_vec();
                            instanced_surfaces_bytes_offset.push(instanced_surfaces_bytes_offset.last().unwrap() + bytes.len() as u64);
                            bytes
                        }).collect::<Vec<u8>>().as_slice(),
                        render_context.inner.window_context.present_queue.get_family_index(),
                    )
                    .await
                    .unwrap();
                // upload surface information
                frame
                    .surface_buffer
                    .upload_to_buffer(
                        &render_context.inner.immediate_submit,
                        surfaces.iter().flat_map(|surface| {
                            bytemuck::bytes_of(surface)
                        }).copied().collect::<Vec<u8>>().as_slice(),
                        render_context.inner.window_context.present_queue.get_family_index(),
                    )
                    .await
                    .unwrap();
                // upload transform information
                frame
                    .transform_buffer
                    .upload_to_buffer(
                        &render_context.inner.immediate_submit,
                        transforms.iter().flat_map(|transform| {
                            bytemuck::bytes_of(transform)
                        }).copied().collect::<Vec<u8>>().as_slice(),
                        render_context.inner.window_context.present_queue.get_family_index(),
                    )
                    .await
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
                    draw_id: 0
                };
                for (index, instancing) in instancing_information.iter().enumerate()
                {
                    let index_buffer = buffers.get_loaded_from_asset_handle(&asset_surfaces[instancing.surface as usize].index_buffer).unwrap();
                    // push new constants
                    push_constant.instanced_surface_info = frame.instanced_buffer.get_buffer().address() + instanced_surfaces_bytes_offset[index] as vk::DeviceAddress;
                    push_constant.draw_id = index as u64;
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
                                    *index_buffer.buffer.as_raw(),
                                    0,
                                    vk::IndexType::UINT32,
                                );
                            render_context
                                .inner
                                .device
                                .get_handle()
                                .cmd_draw_indexed_indirect(
                                    recording.handle(),
                                    unsafe { *frame.indirect_buffer.get_buffer().as_raw() },
                                    0,
                                    1,
                                    size_of::<vk::DrawIndexedIndirectCommand>() as u32,
                                ); // tightly packed :)
                        }
                }
                dynamic_rendering.end_rendering();
            }
            CommandBufferState::Executable(_) => {
                panic!("Mesh recording invalid cmd buffer state")
            }
        };
    }
}
