use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::ash::vk::CommandBuffer;
use dagal::command::CommandBufferState;
use dagal::traits::AsRaw;
use std::mem::swap;
use std::ptr;
use std::ptr::write;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Grabs the final present image and draws it
pub fn present_system_begin(
    mut frame_count: becs::Res<'_, super::frame_number::FrameCount>,
    render_context: becs::Res<'_, super::render_context::RenderContext>,
) {
    let frame_count = frame_count.0.clone();
    let render_context = render_context.clone();
    tokio::runtime::Handle::current().block_on(async move {
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
        let frame_number = frame_count.load(Ordering::Acquire);
        println!("Starting frame {frame_number}");
        let mut frame_guard = surface_context.frames
            [frame_number % surface_context.frames_in_flight]
            .lock()
            .await;
        let mut frame = &mut *frame_guard;
        unsafe {
            // wait for frame to finish rendering before rendering again
            frame.render_fence.wait(u64::MAX).unwrap();
            frame.render_fence.reset().unwrap();
        }
        // wait until semaphore is ready
        let swapchain_image_index = surface_context.swapchain.next_image_index(
            u64::MAX,
            Some(&frame.swapchain_semaphore),
            None,
        );
        let swapchain_image_index = match swapchain_image_index {
            Ok(index) => index,
            Err(e) => {
                tracing::error!("Failed to acquire next swapchain image due to: {e}");
                return;
            }
        };
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
            &frame.queue,
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
        }
    });
}

pub fn present_system_end(
    mut frame_count: becs::Res<'_, super::frame_number::FrameCount>,
    render_context: becs::Res<'_, super::render_context::RenderContext>,
) {
    let window_context = render_context.inner.window_context.clone();
    let frame_count = frame_count.0.clone();
    println!("Ending frame number {:?}", frame_count);
    tokio::runtime::Handle::current().block_on(async move {
        if window_context.surface_context.read().await.is_none() {
            return;
        }
        let surface_guard = window_context.surface_context.read().await;
        let surface_context = surface_guard.as_ref().unwrap();
        let frame_number = frame_count.load(Ordering::Acquire);
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
                CommandBufferState::Recording(r) => r,
                _ => panic!("Expected frame command buffer to be in executable state, got other"),
            };
            frame.draw_image.transition(
                cmd_recording,
                &window_context.present_queue,
                vk::ImageLayout::GENERAL,
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
        }
        {
            let submit_info = {
                // executable swapchain
                frame.command_buffer.end().unwrap();
                let cmd_executable = match &frame.command_buffer {
                    CommandBufferState::Executable(e) => e,
                    _ => panic!(
                        "Expected frame command buffer to be in executable state, found other"
                    ),
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
                            .acquire_queue_lock()
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
                            .acquire_queue_lock()
                            .await
                            .unwrap(),
                        &present_info,
                    ) {
                        Ok(_) => {}
                        Err(error) => match error {
                            vk::Result::ERROR_OUT_OF_DATE_KHR => {
                                return;
                            }
                            e => panic!("Error in queue present {:?}", e),
                        },
                    }
                }
            }
        }
        // progress to next frame
        frame_count.fetch_add(1, Ordering::Release);
        println!("Finished frame {frame_number}");
    });
}
