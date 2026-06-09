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

use crate::{contexts, timer::Timer};

static START_TIME: OnceLock<Instant> = OnceLock::new();

/// Main render loop that clears the swapchain with a fade effect
pub fn render_system<A: Allocator + 'static>(
    mut gpu: NonSendMut<contexts::RenderGpu<A>>,
    mut timer: ResMut<Timer>,
) {
    let gpu = gpu.as_mut();

    if gpu.swapchain.image_count() == 0
        || gpu.swapchain.extent.width == 0
        || gpu.swapchain.extent.height == 0
    {
        return;
    }
    let now = Instant::now();
    timer.last_recorded = Some(now);
    let _trace_frame = tracy_client::Client::running()
        .map(|client| client.non_continuous_frame(tracy_client::frame_name!("Frame")));
    let _render_system_span = tracy_client::span!("Render System");
    _render_system_span.emit_value(gpu.swapchain.image_count() as u64);
    if gpu.present.present_semaphores.is_empty() {
        return;
    }
    let frame_slot = gpu.present.frame_index as usize;

    let _prepare_span = tracy_client::span!("Prepare Frame");
    let image_index = {
        let frame = &mut gpu.present.frames[frame_slot];
        frame.render_fence.wait(u64::MAX).unwrap();
        match gpu.swapchain.swapchain.next_image_index(
            u64::MAX,
            Some(&frame.swapchain_semaphore),
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
        }
    };

    let present_signal = gpu.present.present_semaphores[image_index as usize]
        .submit_info(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT);
    let present_wait_semaphore =
        unsafe { *gpu.present.present_semaphores[image_index as usize].as_raw() };

    let frame = &mut gpu.present.frames[frame_slot];
    frame.render_fence.reset().unwrap();
    let command_buffer = frame.command_pool.allocate(1).unwrap().pop().unwrap();
    let recording = command_buffer
        .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
        .unwrap();

    let swapchain_image = gpu
        .swapchain
        .frame_mut(image_index as usize)
        .expect("swapchain image missing");
    swapchain_image.image.transition(
        &recording,
        &gpu.core.queues.present,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::GENERAL,
    );
    unsafe {
        const SPEED: f32 = 0.125;
        let start = *START_TIME.get_or_init(|| now);
        let elapsed = now.duration_since(start).as_secs_f32();
        let mix = 0.5 + 0.5 * (elapsed * SPEED * TAU).sin();
        let clear_color = [mix / 2.0, 0.0, 0.0, 1.0];
        recording.get_device().get_handle().cmd_clear_color_image(
            *recording.as_raw(),
            *swapchain_image.image.as_raw(),
            vk::ImageLayout::GENERAL,
            &vk::ClearColorValue {
                float32: clear_color,
            },
            &[Image::<A>::image_subresource_range(
                vk::ImageAspectFlags::COLOR,
            )],
        );
    }
    swapchain_image.image.transition(
        &recording,
        &gpu.core.queues.present,
        vk::ImageLayout::GENERAL,
        vk::ImageLayout::PRESENT_SRC_KHR,
    );

    let executable = recording.end().unwrap();
    drop(_prepare_span);
    let _submit_span = tracy_client::span!("Submit Frame");

    let submit_info = executable.submit_info();
    let wait_info = [frame
        .swapchain_semaphore
        .submit_info(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)];
    let submit_batch =
        CommandBufferExecutable::submit_info_sync(&[submit_info], &wait_info, &[present_signal]);

    let queue_handle = unsafe { *gpu.core.queues.present.as_raw() };
    let _command_buffer = executable
        .submit(queue_handle, &[submit_batch], unsafe {
            *frame.render_fence.as_raw()
        })
        .unwrap();

    let present_info = vk::PresentInfoKHR {
        s_type: vk::StructureType::PRESENT_INFO_KHR,
        p_next: ptr::null(),
        wait_semaphore_count: 1,
        p_wait_semaphores: &present_wait_semaphore,
        swapchain_count: 1,
        p_swapchains: gpu.swapchain.swapchain_handle(),
        p_image_indices: &image_index,
        p_results: ptr::null_mut(),
        _marker: Default::default(),
    };

    let present_result = unsafe {
        gpu.swapchain
            .swapchain
            .get_ext()
            .queue_present(queue_handle, &present_info)
    }
    .map(|_| ());
    gpu.present.frame_index = (gpu.present.frame_index + 1) % (gpu.present.frames.len() as u64);
    match present_result {
        Ok(()) => {}
        Err(vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(vk::Result::SUBOPTIMAL_KHR) => {
            tracing::warn!("Swapchain out of date or suboptimal on present. Resizing imminent.");
        }
        Err(err) => tracing::error!(?err, "queue_present failed"),
    }
}
