use crate::prelude as dare;
use crate::prelude::render;
use crate::render2::physical_resource;
use bevy_ecs::prelude as becs;
use bevy_ecs::prelude::Query;
use dagal::allocators::GPUAllocatorImpl;
use dagal::ash::vk;
use dagal::command::CommandBufferState;
use dagal::traits::AsRaw;
use std::ptr;

/// Grabs the final present image and draws it
pub fn present_system_begin(
    mut frame_count: becs::ResMut<'_, super::frame_number::FrameCount>,
    device_context: becs::Res<'_, crate::render2::contexts::DeviceContext>,
    graphics_context: becs::Res<'_, crate::render2::contexts::GraphicsContext>,
    transfer_context: becs::Res<'_, crate::render2::contexts::TransferContext>,
    mut window_context: becs::ResMut<'_, crate::render2::contexts::WindowContext>,
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
        // Batch update all physical resource storages for better performance
        let update_span = tracy_client::span!("physical_resources_update");
        textures.update();
        samplers.update();
        buffers.update();
        update_span.emit_text("Physical resources updated");
        
        let present_queue = window_context.present_queue.clone();
        let surface_context = match window_context.surface_context.as_mut() {
            Some(surface) => surface,
            None => return,
        };
        let frame_number = frame_count.get();
        #[cfg(feature = "tracing")]
        tracing::trace!("Starting frame {frame_number}");
        let frame = &mut surface_context.frames[frame_number % surface_context.frames_in_flight];
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
        match swapchain_image_index {
            Ok(swapchain_image_index) => {
                surface_context.swapchain_image_index = swapchain_image_index;
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
                    &present_queue,
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
                        &present_queue,
                        vk::ImageLayout::GENERAL,
                        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    );
                    frame.depth_image.transition(
                        recording_cmd,
                        &present_queue,
                        vk::ImageLayout::UNDEFINED,
                        vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
                    );
                }
                // mesh render
                super::mesh_render_system::mesh_render(
                    frame_number,
                    &device_context,
                    &graphics_context,
                    &transfer_context,
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
                    frame_number,
                    &present_queue,
                    surface_context,
                    swapchain_image_index,
                    &mut textures,
                    &mut buffers,
                )
                .await;
            }
            Err(e) => {
                tracing::error!("Failed to acquire next swapchain image due to: {e}");
                // early return
                // TODO: Implement new_swapchain_requested with separate contexts
                // render_context
                //     .inner
                //     .new_swapchain_requested
                //     .store(true, Ordering::Release);
                tracing::warn!("Swapchain recreation not yet implemented with separate contexts");
                return;
            }
        };

        // progress to next frame
        frame_count.increment();
    });
}

pub async fn present_system_end(
    frame_count: usize,
    present_queue: &dagal::device::Queue,
    surface_context: &mut crate::render2::contexts::SurfaceContext,
    swapchain_image_index: u32,
    _textures: &mut physical_resource::PhysicalResourceStorage<
        physical_resource::RenderImage<GPUAllocatorImpl>,
    >,
    buffers: &mut physical_resource::PhysicalResourceStorage<
        physical_resource::RenderBuffer<GPUAllocatorImpl>,
    >,
) {
    #[cfg(feature = "tracing")]
    tracing::trace!("Submitting frame {:?}", frame_count);
    let frame = &mut surface_context.frames[frame_count % surface_context.frames_in_flight];
    let swapchain_image = &mut surface_context.swapchain_images[swapchain_image_index as usize];
    {
        let cmd_recording = match &frame.command_buffer {
            CommandBufferState::Recording(r) => r,
            _ => panic!("Expected frame command buffer to be in executable state, got other"),
        };
        frame.draw_image.transition(
            cmd_recording,
            present_queue,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        );
        swapchain_image.transition(
            cmd_recording,
            present_queue,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );
        // copy from draw into swapchain
        swapchain_image.copy_from(cmd_recording, &frame.draw_image);
        swapchain_image.transition(
            cmd_recording,
            present_queue,
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
                    *present_queue.acquire_queue_async().await.unwrap(),
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
                    *present_queue.acquire_queue_async().await.unwrap(),
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
    #[cfg(feature = "tracing")]
    tracing::trace!("Finished frame {frame_count}");
}
