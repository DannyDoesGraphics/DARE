use crate::prelude as dare;
use crate::prelude::render::components::RenderBuffer;
use crate::prelude::render::util::GPUResourceTable;
use crate::render2::c::CPushConstant;
use crate::render2::render_assets::{RenderAssetServer, RenderAssetsStorage};
use crate::render2::resources::MeshBuffer;
use bevy_ecs::change_detection::Res;
use bevy_ecs::prelude as becs;
use bevy_ecs::prelude::Query;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::ash::vk::Handle;
use dagal::command::command_buffer::CmdBuffer;
use dagal::command::CommandBufferState;
use dagal::pipelines::Pipeline;
use dagal::traits::AsRaw;
use std::hash::{Hash, Hasher};
use tokio::task;

pub async fn mesh_render(
    frame_number: usize,
    render_context: super::render_context::RenderContext,
    camera: &dare::render::components::camera::Camera,
    frame: &mut super::frame::Frame,
    buffers: Res<'_, RenderAssetsStorage<RenderBuffer<GPUAllocatorImpl>>>,
    mut mesh_buffer: becs::ResMut<'_, MeshBuffer<GPUAllocatorImpl>>,
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
                mesh_buffer
                    .flush(&render_context.inner.immediate_submit.clone(), &buffers)
                    .await;
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
                let mut last_index_address: Option<vk::DeviceAddress> = None;
                for (mesh, _) in mesh_buffer.mesh_container.iter() {
                    let surface = mesh.surface.clone().upgrade();
                    if surface.is_none() {
                        continue;
                    }
                    let surface = surface.unwrap();
                    if buffers.get(&surface.vertex_buffer.id()).is_none()
                        || buffers.get(&surface.index_buffer.id()).is_none()
                    {
                        // try to load them in
                        if frame_number % 1024 == 0 {
                            if buffers.get(&surface.vertex_buffer.id()).is_none() {
                                println!("Failed: {:?}", surface.vertex_buffer.id());
                            }
                            if buffers.get(&surface.vertex_buffer.id()).is_none() {
                                println!("Failed: {:?}", surface.index_buffer.id());
                            }
                        }
                        continue;
                    }
                    // calculate visibility
                    let model = mesh.transform.get_transform_matrix();
                    let camera_view = camera.get_view_matrix();
                    let camera_proj = camera.get_projection(
                        frame.image_extent.width as f32 / frame.image_extent.height as f32,
                    );
                    let camera_view_proj = camera_proj * camera_view;

                    if !mesh
                        .bounding_box
                        .visible_in_frustum(model, camera_view_proj)
                    {
                        continue;
                    }

                    frame
                        .resources
                        .insert(surface.vertex_buffer.clone().into_untyped_handle());
                    frame
                        .resources
                        .insert(surface.index_buffer.clone().into_untyped_handle());
                    let vertex_buffer = buffers.get(&surface.vertex_buffer.id()).unwrap();
                    let index_buffer = buffers.get(&surface.index_buffer.id()).unwrap();

                    unsafe {
                        if last_index_address
                            .as_ref()
                            .map(|id| id.clone() != index_buffer.buffer.address())
                            .unwrap_or(true)
                        {
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
                            last_index_address = Some(index_buffer.buffer.address());
                        }
                        let view_proj = camera_view_proj * model;
                        let push_constant = CPushConstant {
                            transform: view_proj.to_cols_array(),
                            vertex_buffer: vertex_buffer.address(),
                        };
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

                        render_context.inner.device.get_handle().cmd_draw_indexed(
                            recording.handle(),
                            surface.index_count as u32,
                            1,
                            0,
                            0,
                            0,
                        )
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
