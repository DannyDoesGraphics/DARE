use crate::prelude as dare;
use crate::prelude::render;
use crate::render2::physical_resource;
use crate::render2::render_assets::RenderAssetsStorage;
use bevy_ecs::prelude as becs;
use bevy_ecs::prelude::Query;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::ash::vk::CommandBuffer;
use dagal::command::CommandBufferState;
use dagal::traits::AsRaw;
use std::mem::swap;
use std::ptr;
use std::ptr::write;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::MutexGuard;

/// Grabs the final present image and draws it
pub fn present_system_begin(
    frame_count: becs::ResMut<'_, super::frame_number::FrameCount>,
    render_context: becs::Res<'_, super::render_context::RenderContext>,
    rt: becs::Res<'_, dare::concurrent::BevyTokioRunTime>,
    surfaces: Query<
        '_,
        '_,
        (
            becs::Entity,
            &dare::engine::components::Surface,
            Option<&dare::engine::components::Material>,
            &render::components::BoundingBox,
            &dare::physics::components::Transform,
            &dare::engine::components::Name,
        ),
    >,
    mut textures: becs::ResMut<
        '_,
        physical_resource::PhysicalResourceStorage<
            physical_resource::RenderImage<GPUAllocatorImpl>,
        >,
    >,
    mut samplers: becs::ResMut<
        physical_resource::PhysicalResourceStorage<dare::asset2::assets::SamplerAsset>,
    >,
    mut buffers: becs::ResMut<
        '_,
        physical_resource::PhysicalResourceStorage<
            physical_resource::RenderBuffer<GPUAllocatorImpl>,
        >,
    >,
    camera: becs::Res<'_, render::components::camera::Camera>,
) {
    rt.clone().runtime.block_on(async {
        let frame_count = frame_count.clone();
        let render_context = render_context.clone();
        let mut surface_guard = render_context
            .inner
            .window_context
            .surface_context
            .write()
            .unwrap();
        let surface = surface_guard.as_mut();
        if surface.is_none() {
            return;
        }
        let surface_context = surface.unwrap();
        let frame_number = frame_count.load(Ordering::Acquire);
        #[cfg(feature = "tracing")]
        tracing::trace!("Starting frame {frame_number}");
        let mut frame_guard = surface_context.frames
            [frame_number % surface_context.frames_in_flight]
            .lock()
            .await;
        let frame = &mut *frame_guard;
        // wait until semaphore is ready
        // wait for frame to finish rendering before rendering again
        frame.render_fence.wait(u64::MAX).unwrap();
        frame.render_fence.reset().unwrap();
        // drop all resource handles
        frame.resources.clear();
        // drop all staging buffers
        frame.staging_buffers.clear();
        let swapchain_image_index = surface_context.swapchain.next_image_index(
            u64::MAX,
            Some(&frame.swapchain_semaphore),
            None,
        );
        let _swapchain_image_index = match swapchain_image_index {
            Ok(swapchain_image_index) => {
                *surface_context.swapchain_image_index.write().await = swapchain_image_index;
                //let swapchain_image = &window_context.swapchain_images[swapchain_image_index as usize];
                // Reset and set command buffer into executable
                frame
                    .command_buffer
                    .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
                    .unwrap();
                let recording_cmd = match &frame.command_buffer {
                    CommandBufferState::Recording(cmd) => cmd,
                    _ => panic!("Expected recording command buffer, got other"),
                };
                frame.draw_image.transition(
                    recording_cmd,
                    &render_context.inner.window_context.present_queue,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::GENERAL,
                );

                // TODO: remove test temp code
                unsafe {
                    surface_context
                        .allocator
                        .device()
                        .get_handle()
                        .cmd_clear_color_image(
                            **recording_cmd,
                            *frame.draw_image.as_raw(),
                            vk::ImageLayout::GENERAL,
                            &vk::ClearColorValue {
                                float32: [
                                    ((((frame_number as f64) / 200.0).cos() as f32) + 1.0) / 2.0,
                                    0.0,
                                    0.0,
                                    0.0,
                                ],
                            },
                            &[
                                dagal::resource::Image::<GPUAllocatorImpl>::image_subresource_range(
                                    vk::ImageAspectFlags::COLOR,
                                ),
                            ],
                        );

                    // transition
                    // transition image states first
                    frame.draw_image.transition(
                        recording_cmd,
                        &render_context.inner.window_context.present_queue,
                        vk::ImageLayout::GENERAL,
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    );
                    frame.depth_image.transition(
                        recording_cmd,
                        &render_context.inner.window_context.present_queue,
                        vk::ImageLayout::UNDEFINED,
                        vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
                    );
                }
                // mesh render
                super::mesh_render_system::mesh_render(
                    frame_number,
                    render_context.clone(),
                    &camera,
                    frame,
                    surfaces,
                    &mut textures,
                    &mut samplers,
                    &mut buffers,
                )
                .await;
                // end present
                present_system_end(
                    frame_count.clone(),
                    render_context.clone(),
                    surface_context,
                    frame,
                    swapchain_image_index,
                    &mut textures,
                    &mut buffers,
                )
                .await;
            }
            Err(e) => {
                tracing::error!("Failed to acquire next swapchain image due to: {e}");
                // early return
                render_context
                    .inner
                    .new_swapchain_requested
                    .store(true, Ordering::Release);
                return;
            }
        };
    });
}

pub async fn present_system_end(
    frame_count: super::frame_number::FrameCount,
    render_context: super::render_context::RenderContext,
    surface_context: &super::surface_context::SurfaceContext,
    mut frame: &mut super::frame::Frame,
    swapchain_image_index: u32,
    textures: &mut physical_resource::PhysicalResourceStorage<
        physical_resource::RenderImage<GPUAllocatorImpl>,
    >,
    buffers: &mut physical_resource::PhysicalResourceStorage<
        physical_resource::RenderBuffer<GPUAllocatorImpl>,
    >,
) {
    let window_context = render_context.inner.window_context.clone();
    let frame_count = frame_count.0.clone();

    #[cfg(feature = "tracing")]
    tracing::trace!("Submitting frame {:?}", frame_count);
    let mut swapchain_image: std::sync::MutexGuard<dagal::resource::Image<GPUAllocatorImpl>> =
        surface_context.swapchain_images[swapchain_image_index as usize]
            .lock()
            .unwrap();
    {
        let cmd_recording = match &frame.command_buffer {
            CommandBufferState::Recording(r) => r,
            _ => panic!("Expected frame command buffer to be in executable state, got other"),
        };
        frame.draw_image.transition(
            cmd_recording,
            &window_context.present_queue,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        );
        swapchain_image.transition(
            cmd_recording,
            &window_context.present_queue,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );
        // copy from draw into swapchain
        swapchain_image.copy_from(cmd_recording, &frame.draw_image);
        swapchain_image.transition(
            cmd_recording,
            &window_context.present_queue,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
        );
        drop(swapchain_image);
    }
    {
        let submit_info = {
            // executable swapchain
            frame.command_buffer.end().unwrap();
            let cmd_executable = match &frame.command_buffer {
                CommandBufferState::Executable(e) => e,
                _ => panic!("Expected frame command buffer to be in executable state, found other"),
            };
            let submit_info = cmd_executable.submit_info();
            dagal::command::CommandBufferExecutable::submit_info_sync(
                &[submit_info],
                &[frame
                    .swapchain_semaphore
                    .submit_info(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)],
                &[frame
                    .render_semaphore
                    .submit_info(vk::PipelineStageFlags2::ALL_GRAPHICS)],
            )
        };
        {
            frame
                .command_buffer
                .submit(
                    *window_context
                        .present_queue
                        .acquire_queue_async()
                        .await
                        .unwrap(),
                    &[submit_info],
                    unsafe { *frame.render_fence.as_raw() },
                )
                .unwrap();
            let present_info = vk::PresentInfoKHR {
                s_type: vk::StructureType::PRESENT_INFO_KHR,
                p_next: ptr::null(),
                wait_semaphore_count: 1,
                p_wait_semaphores: unsafe { frame.render_semaphore.as_raw() },
                swapchain_count: 1,
                p_swapchains: unsafe { surface_context.swapchain.as_raw() },
                p_image_indices: &swapchain_image_index,
                p_results: ptr::null_mut(),
                _marker: Default::default(),
            };
            unsafe {
                match surface_context.swapchain.get_ext().queue_present(
                    *window_context
                        .present_queue
                        .acquire_queue_async()
                        .await
                        .unwrap(),
                    &present_info,
                ) {
                    Ok(_) => {}
                    Err(error) => match error {
                        vk::Result::ERROR_OUT_OF_DATE_KHR => {
                            println!("Old swapchain found");
                            return;
                        }
                        e => panic!("Error in queue present {:?}", e),
                    },
                }
            }
        }
    }
    // progress to next frame + update physical storage
    buffers.update();
    //textures.update();
    frame_count.fetch_add(1, Ordering::AcqRel);
    #[cfg(feature = "tracing")]
    tracing::trace!("Finished frame {frame_number}");
}
