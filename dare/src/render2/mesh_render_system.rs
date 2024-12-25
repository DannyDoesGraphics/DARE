use crate::prelude as dare;
use crate::prelude::render::components::RenderBuffer;
use crate::prelude::render::util::GPUResourceTable;
use crate::render2::c::CPushConstant;
use crate::render2::resources::RenderSurfaceManager;
use bevy_ecs::prelude as becs;
use bevy_ecs::prelude::Query;
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
use tokio::task;

pub async fn mesh_render(
    frame_number: usize,
    render_context: super::render_context::RenderContext,
    camera: &dare::render::components::camera::Camera,
    frame: &mut super::frame::Frame,
    buffers: becs::Res<
        '_,
        dare::render::render_assets::RenderAssetsStorage<RenderBuffer<GPUAllocatorImpl>>,
    >,
    mut surface_buffer: becs::ResMut<'_, RenderSurfaceManager>,
    mesh_link: becs::Res<'_, dare::render::render_assets::MeshLink>,
) {
    #[cfg(feature = "tracing")]
    tracing::trace!("Rendering meshes into {frame_number}");
    {
        let cmd_recording = match &frame.command_buffer {
            CommandBufferState::Ready(_) => {
                panic!("Mesh recording invalid cmd buffer state")
            }
            CommandBufferState::Recording(recording) => {
                // flush buffers for upload
                let mut surfaces_used: HashMap<
                    (
                        dare::engine::components::Surface,
                        Option<dare::engine::components::Material>,
                    ),
                    dare::render::c::InstancedSurfacesInfo,
                > = HashMap::new();
                for (_, mesh) in mesh_link.iter() {
                    // if surface is still actively used?
                    let surface = mesh.surface.clone().upgrade();
                    if surface.is_none() {
                        continue;
                    }
                    let surface = surface.unwrap();
                    // test visibility
                    let model_transform = mesh.transform.get_transform_matrix();
                    let camera_view = camera.get_view_matrix();
                    let camera_proj = camera.get_projection(
                        frame.image_extent.width as f32 / frame.image_extent.height as f32,
                    );
                    let mut camera_view_proj = camera_proj * camera_view;
                    camera_view_proj.y_axis.y *= -1.0;
                    if !mesh
                        .bounding_box
                        .visible_in_frustum(model_transform, camera_view_proj)
                    {
                        continue;
                    }
                    /*
                    surfaces_used
                        .entry((surface.clone(), None))
                        .and_modify(|instanced_info| {
                            instanced_info.instances += 1;
                        })
                        .or_insert({
                            let surface_index = surface_buffer
                                .surface_hashes
                                .get(&surface.clone())
                                .map(|slot| slot.id())
                                .expect("Surface does not exist in surface buffer");
                            dare::render::c::InstancedSurfacesInfo {
                                surface: surface_buffer.growable_buffer.get_buffer().address()
                                    + (size_of::<dare::render::c::InstancedSurfacesInfo>()
                                        * surface_index)
                                        as vk::DeviceSize,
                                instances: 1,
                            }
                        });
                     */
                }
                // generate indirect calls
                let indirect_calls: Vec<vk::DrawIndexedIndirectCommand> = surfaces_used
                    .iter()
                    .map(|((surface, _), instanced)| vk::DrawIndexedIndirectCommand {
                        index_count: surface.index_count as u32,
                        instance_count: instanced.instances as u32,
                        first_index: 0,
                        vertex_offset: 0,
                        first_instance: 0,
                    })
                    .collect();
                // if nothing to render, skip
                if indirect_calls.is_empty() {
                    return;
                }
                // we only need the instanced info
                let instanced_surfaces_info: Vec<(
                    dare::engine::components::Surface,
                    dare::render::c::InstancedSurfacesInfo,
                )> = surfaces_used
                    .iter()
                    .map(|((s, _), v)| {
                        (s.clone(), v.clone())
                    })
                    .collect();
                let instanced_surfaces_bytes: Vec<u8> = instanced_surfaces_info
                    .iter()
                    .flat_map(|(_, instanced_surface)| {
                        bytemuck::bytes_of(instanced_surface).iter().copied().collect::<Vec<u8>>()
                    })
                    .collect::<Vec<u8>>();
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
                // upload surfaces
                frame
                    .instanced_buffer
                    .upload_to_buffer(
                        &render_context.inner.immediate_submit,
                        instanced_surfaces_bytes.as_slice(),
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
                    draw_id: 0
                };
                for (index, (surface, _instanced_info)) in instanced_surfaces_info.iter().enumerate()
                {
                    if let Some(index_buffer) = buffers.get(&surface.index_buffer.id()) {
                        if buffers.get(&surface.vertex_buffer.id()).is_none() {
                            continue;
                        }
                        // push new constants
                        push_constant.instanced_surface_info = frame.instanced_buffer.get_buffer().address();
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
                }
                dynamic_rendering.end_rendering();
            }
            CommandBufferState::Executable(_) => {
                panic!("Mesh recording invalid cmd buffer state")
            }
        };
    }
}
