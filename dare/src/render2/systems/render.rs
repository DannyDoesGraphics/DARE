use std::{f32::consts::TAU, ptr, sync::OnceLock, time::Instant};

use bevy_ecs::prelude::*;
use dagal::{
    DagalError,
    allocators::Allocator,
    ash::vk,
    command::{CommandBufferExecutable, command_buffer::CmdBuffer},
    resource::Image,
    traits::AsRaw,
};

use crate::render2::{contexts, timer::Timer};

static START_TIME: OnceLock<Instant> = OnceLock::new();

/// Main render loop that clears the swapchain with a fade effect
pub fn render_system<A: Allocator + 'static>(
    core_context: Res<contexts::CoreContext>,
    mut swapchain_context: ResMut<contexts::SwapchainContext<A>>,
    mut present_context: ResMut<contexts::PresentContext>,
    mut timer: ResMut<Timer>,
) {
    tokio::runtime::Handle::current().block_on(async move {
        if let Err(err) = swapchain_context.ensure_frames() {
            tracing::warn!(?err, "Swapchain frames unavailable");
            return;
        }
        if swapchain_context.image_count() == 0
            || swapchain_context.extent.width == 0
            || swapchain_context.extent.height == 0
        {
            return;
        }
        let now = Instant::now();
        timer.last_recorded = Some(now);
        const SPEED: f32 = 0.125;
        let start = *START_TIME.get_or_init(|| now);
        let elapsed = now.duration_since(start).as_secs_f32();
        let mix = 0.5 + 0.5 * (elapsed * SPEED * TAU).sin();
        let clear_color = [mix / 2.0, 0.0, 0.0, 1.0];

        if let Err(err) = present_context.in_flight_fence.wait(u64::MAX) {
            tracing::error!(?err, "Failed to wait for in-flight fence");
            return;
        }
        let image_index = match swapchain_context.swapchain.next_image_index(
            u64::MAX,
            Some(&present_context.image_available_semaphore),
            None,
        ) {
            Ok(index) => index,
            Err(DagalError::VkError(vk::Result::ERROR_OUT_OF_DATE_KHR))
            | Err(DagalError::VkError(vk::Result::SUBOPTIMAL_KHR)) => {
                return;
            }
            Err(err) => {
                tracing::error!(?err, "Failed to acquire next swapchain image");
                return;
            }
        };
        if (image_index as usize) >= swapchain_context.image_count() {
            tracing::warn!(image_index, "Swapchain reported invalid image index");
            return;
        }

        let command_buffer = match present_context.command_buffer.take() {
            Some(buffer) => buffer,
            None => match present_context
                .command_pool
                .allocate(1)
                .ok()
                .and_then(|mut buffers| buffers.pop())
            {
                Some(buffer) => buffer,
                None => {
                    tracing::error!("Failed to allocate command buffer for presentation");
                    return;
                }
            },
        };

        let recording = match command_buffer.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT) {
            Ok(recording) => recording,
            Err(invalid) => match invalid.reset(None) {
                Ok(buffer) => {
                    present_context.command_buffer = Some(buffer);
                    return;
                }
                Err(err) => {
                    tracing::error!(?err, "Failed to reset present command buffer");
                    return;
                }
            },
        };

        let frame = swapchain_context
            .frame_mut(image_index as usize)
            .expect("swapchain image missing");

        frame.image.transition(
            &recording,
            &core_context.present_queue,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );
        unsafe {
            recording.get_device().get_handle().cmd_clear_color_image(
                *recording.as_raw(),
                *frame.image.as_raw(),
                vk::ImageLayout::GENERAL,
                &vk::ClearColorValue {
                    float32: clear_color,
                },
                &[Image::<A>::image_subresource_range(
                    vk::ImageAspectFlags::COLOR,
                )],
            );
        }
        frame.image.transition(
            &recording,
            &core_context.present_queue,
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
        );

        let executable = match recording.end() {
            Ok(executable) => executable,
            Err(err) => {
                tracing::error!(?err, "Failed to end present command buffer");
                present_context.command_buffer = None;
                return;
            }
        };

        let submit_info = executable.submit_info();
        let wait_info = [present_context
            .image_available_semaphore
            .submit_info(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)];
        let signal_info = [present_context
            .render_finished_semaphore
            .submit_info(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)];
        let submit_batch =
            CommandBufferExecutable::submit_info_sync(&[submit_info], &wait_info, &signal_info);

        if let Err(err) = present_context.in_flight_fence.reset() {
            tracing::error!(?err, "Failed to reset in-flight fence");
            present_context.command_buffer = None;
            return;
        }

        let queue_guard = core_context
            .present_queue
            .acquire_queue_async()
            .await
            .unwrap();
        let queue_handle = *queue_guard;
        let fence_handle = unsafe { *present_context.in_flight_fence.as_raw() };
        let command_buffer = match executable.submit(queue_handle, &[submit_batch], fence_handle) {
            Ok(buffer) => buffer,
            Err(invalid) => match invalid.reset(None) {
                Ok(buffer) => buffer,
                Err(err) => {
                    tracing::error!(?err, "Failed to reset command buffer after submit error");
                    present_context.command_buffer = None;
                    return;
                }
            },
        };

        let present_info = vk::PresentInfoKHR {
            s_type: vk::StructureType::PRESENT_INFO_KHR,
            p_next: ptr::null(),
            wait_semaphore_count: 1,
            p_wait_semaphores: unsafe { present_context.render_finished_semaphore.as_raw() },
            swapchain_count: 1,
            p_swapchains: swapchain_context.swapchain_handle(),
            p_image_indices: &image_index,
            p_results: ptr::null_mut(),
            _marker: Default::default(),
        };

        let present_result = unsafe {
            swapchain_context
                .swapchain
                .get_ext()
                .queue_present(*queue_guard, &present_info)
        }
        .map(|_| ());
        drop(queue_guard);
        present_context.command_buffer = Some(command_buffer);

        match present_result {
            Ok(()) => {}
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {}
            Err(err) => tracing::error!(?err, "queue_present failed"),
        }
    });
}
