use std::mem::swap;
use std::ptr;
use std::ptr::write;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::ash::vk::CommandBuffer;
use dagal::command::CommandBufferState;
use dagal::traits::AsRaw;

/// Grabs the final present image and draws it
pub fn present_system_begin<A: Allocator + 'static>(mut frame_count: becs::Res<'_, super::frame_number::FrameCount>, render_context: becs::Res<'_, super::render_context::RenderContext>) {
    let frame_count = frame_count.0.clone();
    let render_context = render_context.clone();
    futures::executor::block_on(async move {
        let surface_guard = render_context.inner.window_context.surface_context.read().await;
        let surface = surface_guard.as_ref();
        if surface.is_none() {
            println!("No surface");
            return;
        }
        let surface_context = surface.unwrap();
        let frame_number = frame_count.fetch_add(0, Ordering::SeqCst);
        let frame = &surface_context.frames[frame_number % surface_context.frames_in_flight];
        unsafe {
            // wait for frame to finish rendering before rendering again
            frame
                .render_fence
                .wait(u64::MAX)
                .unwrap();
        }
        // wait until semaphore is ready
        let swapchain_image_index = surface_context.swapchain.next_image_index(
            u64::MAX,
            Some(&frame.swapchain_semaphore),
            None
        );
        let swapchain_image_index = match swapchain_image_index {
            Ok(index) => index,
            Err(e) => {
                tracing::error!("Failed to acquire next swapchain image due to: {e}");
                return;
            },
        };
        *surface_context.swapchain_image_index.write().await = swapchain_image_index;
        //let swapchain_image = &window_context.swapchain_images[swapchain_image_index as usize];
        // Reset and set command buffer into executable
        {
            let mut write_guard = frame.command_buffer.write().await;
            let recording = match &*write_guard {
                CommandBufferState::Ready(cmd) => {
                    cmd.reset(vk::CommandBufferResetFlags::empty()).unwrap();
                    CommandBufferState::from(
                        cmd.clone().begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT).unwrap()
                    )
                }
                _ => panic!("Expected frame command buffer to be in ready state, got other")
            };
            *write_guard = recording;
        }
        let cmd_guard = frame.command_buffer.blocking_read();
        let cmd = match &*cmd_guard {
            CommandBufferState::Recording(r) => r,
            _ => panic!("Expected frame command buffer to be in executable state, got other")
        };
        frame.draw_image.transition(
            cmd,
            &frame.queue,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL
        );

        // TODO: remove test temp code
        unsafe {
            surface_context.allocator.device()
                           .get_handle()
                           .cmd_clear_color_image(
                              **cmd,
                              *frame.draw_image.as_raw(),
                              vk::ImageLayout::GENERAL,
                              &vk::ClearColorValue {
                                  float32: [0.0, 0.0, 0.0, 0.0],
                              },
                              &[
                                  dagal::resource::Image::<GPUAllocatorImpl>::image_subresource_range(
                                      vk::ImageAspectFlags::COLOR,
                                  ),
                              ],
                          );
        }

        println!("Finished step 1");
    });
}

pub fn present_system_end<A: Allocator + 'static>(mut frame_count: becs::Res<'_, super::frame_number::FrameCount>, render_context: becs::Res<'_, super::render_context::RenderContext>) {
    let window_context = render_context.inner.window_context.clone();
    let frame_count = frame_count.0.clone();
    tokio::task::spawn(async move {
        if window_context.window.read().await.is_none() {
            return;
        }
        let surface_guard = window_context.surface_context.blocking_read();
        let surface_context = surface_guard.as_ref().unwrap();
        let frame_number = frame_count.fetch_add(0, Ordering::SeqCst);
        let frame = &surface_context.frames[frame_number % surface_context.frames_in_flight];
        let swapchain_image_index_guard = surface_context.swapchain_image_index.blocking_read();
        let swapchain_image_index: u32 = *swapchain_image_index_guard;
        let swapchain_image: &dagal::resource::Image<GPUAllocatorImpl> = &surface_context.swapchain_images[swapchain_image_index as usize];
        {
            let cmd_guard = frame.command_buffer.blocking_read();
            let cmd = match &*cmd_guard {
                CommandBufferState::Recording(r) => r,
                _ => panic!("Expected frame command buffer to be in executable state, got other")
            };


            frame.draw_image.transition(
                cmd,
                &window_context.present_queue,
                vk::ImageLayout::GENERAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL
            );
            swapchain_image.transition(
                cmd,
                &window_context.present_queue,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL
            );
            // copy from draw into swapchain
            swapchain_image.copy_from(
                cmd,
                swapchain_image,
            );
            swapchain_image.transition(
                cmd,
                &window_context.present_queue,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            )
        }
        {
            let submit_info = {
                // executable swapchain
                let cmd_guard = {
                    let mut write_guard = frame.command_buffer.blocking_write();
                    write_guard.end().unwrap();
                    write_guard.downgrade()
                };
                let cmd = match &*cmd_guard {
                    CommandBufferState::Executable(e) => e,
                    _ => panic!("Expected frame command buffer to be in executable state, found other")
                };
                let submit_info = cmd.submit_info();
                dagal::command::CommandBufferExecutable::submit_info_sync(
                    &[submit_info],
                    &[frame.swapchain_semaphore.submit_info(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)],
                    &[frame.render_semaphore.submit_info(vk::PipelineStageFlags2::ALL_GRAPHICS)],
                )
            };
            let queue_guard = window_context.present_queue.acquire_queue_lock().await.unwrap();
            let mut cmd_guard = {
                let mut write_guard = frame.command_buffer.write().await;
                write_guard.end().unwrap();
                write_guard
            };

            cmd_guard
                .submit(
                    *queue_guard,
                    &[submit_info],
                    unsafe { *frame.render_fence.as_raw() },
                )
                .unwrap();
            let present_info = vk::PresentInfoKHR {
                s_type: vk::StructureType::PRESENT_INFO_KHR,
                p_next: ptr::null(),
                wait_semaphore_count: 1,
                p_wait_semaphores: unsafe { &*frame.render_semaphore.as_raw() },
                swapchain_count: 1,
                p_swapchains: unsafe { surface_context.swapchain.as_raw() },
                p_image_indices: &swapchain_image_index,
                p_results: ptr::null_mut(),
                _marker: Default::default(),
            };
            unsafe {
                match surface_context
                    .swapchain
                    .get_ext()
                    .queue_present(*queue_guard, &present_info)
                {
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
        // progress to next frame
        frame_count.fetch_add(1, Ordering::SeqCst);
    });;
}