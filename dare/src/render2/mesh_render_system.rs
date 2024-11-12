use crate::prelude as dare;
use crate::render2::c::CPushConstant;
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::ash::vk::Handle;
use dagal::command::command_buffer::CmdBuffer;
use dagal::command::CommandBufferState;
use dagal::pipelines::Pipeline;
use dagal::traits::AsRaw;

pub fn mesh_render(
    frame_count: becs::ResMut<'_, super::frame_number::FrameCount>,
    render_context: becs::Res<'_, super::render_context::RenderContext>,
    rt: becs::Res<'_, dare::concurrent::BevyTokioRunTime>,
    buffers: becs::Res<
        '_,
        dare::render::render_assets::RenderAssetsStorage<
            dare::render::components::RenderBuffer<GPUAllocatorImpl>,
        >,
    >,
    surfaces: becs::Res<'_, dare::render::resource_relationship::Surfaces>,
    bindless: becs::Res<'_, dare::render::util::GPUResourceTable<GPUAllocatorImpl>>,
) {
    tokio::task::block_in_place(move || {
        rt.clone().runtime.block_on(async move {
            let surface_guard = render_context
                .inner
                .window_context
                .surface_context
                .read()
                .await;
            let surface = surface_guard.as_ref();
            if surface.is_none() {
                return;
            }
            let surface_context = surface.unwrap();
            let frame_number = frame_count.0.load(std::sync::atomic::Ordering::Acquire);
            #[cfg(feature = "tracing")]
            tracing::trace!("Rendering meshes into {frame_number}");
            let mut frame_guard = surface_context.frames
                [frame_number % surface_context.frames_in_flight]
                .lock()
                .await;
            let mut frame = &mut *frame_guard;
            let swapchain_image_index_guard = surface_context.swapchain_image_index.read().await;
            let swapchain_image_index: u32 = *swapchain_image_index_guard;
            let swapchain_image: &dagal::resource::Image<GPUAllocatorImpl> =
                &surface_context.swapchain_images[swapchain_image_index as usize];

            {
                let cmd_recording = match &frame.command_buffer {
                    CommandBufferState::Ready(_) => {
                        panic!("Mesh recording invalid cmd buffer state")
                    }
                    CommandBufferState::Recording(recording) => {
                        let present_queue = &render_context.inner.window_context.present_queue;

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
                                    width: swapchain_image.extent().width,
                                    height: swapchain_image.extent().height,
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

                        for surface in surfaces.0.values() {
                            let surface = surface.clone().upgrade();
                            if surface.is_none() {
                                continue;
                            }
                            let surface = surface.unwrap();
                            if buffers.get(&surface.vertex_buffer.id()).is_none()
                                || buffers.get(&surface.index_buffer.id()).is_none()
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
                                render_context
                                    .inner
                                    .device
                                    .get_handle()
                                    .cmd_bind_index_buffer(
                                        recording.handle(),
                                        *index_buffer.buffer.as_raw(),
                                        0,
                                        vk::IndexType::UINT16,
                                    );
                                let model =
                                    glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, 0.0));
                                let view =
                                    glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, -5.0));
                                let mut proj = glam::Mat4::perspective_rh(
                                    70f32.to_radians(),
                                    frame.image_extent.width as f32
                                        / frame.image_extent.height as f32,
                                    10000.0,
                                    0.1,
                                );
                                proj.y_axis.y *= -1.0; // flip
                                let mut view_proj = proj * view * model;
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
        });
    });
}
