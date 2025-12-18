use std::{
    ffi::{c_char, c_uchar},
    ptr,
};

use dagal::allocators::{Allocator, GPUAllocatorImpl};

use dare_window::WindowHandles;

/// Contains core rendering context information
#[derive(Debug, bevy_ecs::resource::Resource)]
pub struct CoreContext {
    pub present_queue: dagal::device::Queue,
    pub queue_allocator: dagal::util::queue_allocator::QueueAllocator,
    pub allocator: GPUAllocatorImpl,
    pub device: dagal::device::LogicalDevice,
    pub physical_device: dagal::device::PhysicalDevice,
    pub instance: dagal::core::Instance,
}

impl CoreContext {
    pub fn new(handles: &WindowHandles) -> anyhow::Result<(Self, dagal::wsi::SurfaceQueried)> {
        use dagal::{
            ash::vk,
            bootstrap::{
                app_info::{Expected, QueueRequest},
                init::ContextInit,
            },
        };

        let (instance, physical_device, surface, device, allocator) =
            dagal::bootstrap::init::WindowedContext::init(
                dagal::bootstrap::app_info::AppSettings {
                    name: "DARE".to_string(),
                    version: 0,
                    engine_name: "DARE".to_string(),
                    engine_version: 0,
                    api_version: (1, 4, 0, 0),
                    enable_validation: true,
                    debug_utils: cfg!(debug_assertions),
                    raw_display_handle: Some(*handles.raw_display_handle),
                    raw_window_handle: Some(*handles.raw_window_handle),
                    surface_format: Some(Expected::Preferred(vk::SurfaceFormatKHR {
                        format: vk::Format::B8G8R8_SRGB,
                        color_space: Default::default(),
                    })),
                    present_mode: Some(Expected::Required(vk::PresentModeKHR::MAILBOX)),
                    gpu_requirements: dagal::bootstrap::app_info::GPURequirements {
                        dedicated: Expected::Required(true),
                        features: vk::PhysicalDeviceFeatures {
                            shader_int64: vk::TRUE,
                            ..Default::default()
                        },
                        features_1: vk::PhysicalDeviceVulkan11Features {
                            s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_FEATURES,
                            variable_pointers: vk::TRUE,
                            variable_pointers_storage_buffer: vk::TRUE,
                            shader_draw_parameters: vk::TRUE,
                            ..Default::default()
                        },
                        features_2: vk::PhysicalDeviceVulkan12Features {
                            s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_2_FEATURES,
                            buffer_device_address: vk::TRUE,
                            descriptor_indexing: vk::TRUE,
                            descriptor_binding_partially_bound: vk::TRUE,
                            descriptor_binding_update_unused_while_pending: vk::TRUE,
                            descriptor_binding_sampled_image_update_after_bind: vk::TRUE,
                            descriptor_binding_storage_image_update_after_bind: vk::TRUE,
                            descriptor_binding_uniform_buffer_update_after_bind: vk::TRUE,
                            shader_storage_buffer_array_non_uniform_indexing: vk::TRUE,
                            shader_sampled_image_array_non_uniform_indexing: vk::TRUE,
                            shader_storage_image_array_non_uniform_indexing: vk::TRUE,
                            runtime_descriptor_array: vk::TRUE,
                            scalar_block_layout: vk::TRUE,
                            timeline_semaphore: vk::TRUE,
                            descriptor_binding_storage_buffer_update_after_bind: vk::TRUE,
                            ..Default::default()
                        },
                        features_3: vk::PhysicalDeviceVulkan13Features {
                            s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_3_FEATURES,
                            dynamic_rendering: vk::TRUE,
                            synchronization2: vk::TRUE,
                            ..Default::default()
                        },
                        device_extensions: vec![
                            Expected::Required(
                                dagal::ash::khr::swapchain::NAME
                                    .to_string_lossy()
                                    .to_string(),
                            ),
                            Expected::Preferred(
                                dagal::ash::ext::debug_utils::NAME
                                    .to_string_lossy()
                                    .to_string(),
                            ),
                        ],
                        queues: vec![
                            QueueRequest {
                                strict: false,
                                queue_type: vec![Expected::Required(
                                    vk::QueueFlags::GRAPHICS
                                        | vk::QueueFlags::TRANSFER
                                        | vk::QueueFlags::COMPUTE,
                                )]
                                .into(),
                                count: Expected::Required(2),
                            },
                            QueueRequest {
                                strict: false,
                                queue_type: vec![Expected::Required(vk::QueueFlags::TRANSFER)]
                                    .into(),
                                count: Expected::Preferred(u32::MAX),
                            },
                        ],
                    },
                },
            )?;

        // Retrieve transfer queues
        let all_queues = physical_device
            .get_active_queues()
            .iter()
            .map(|queue_info| unsafe {
                device.get_queue(
                    &vk::DeviceQueueInfo2 {
                        s_type: vk::StructureType::DEVICE_QUEUE_INFO_2,
                        p_next: ptr::null(),
                        flags: vk::DeviceQueueCreateFlags::empty(),
                        queue_family_index: queue_info.family_index,
                        queue_index: queue_info.index,
                        _marker: Default::default(),
                    },
                    queue_info.queue_flags,
                    queue_info.strict,
                    queue_info.can_present,
                )
            })
            .collect::<Vec<dagal::device::Queue>>();

        let (present_queue, remaining_queues) = {
            let mut present_queue = None;
            let mut remaining = Vec::new();

            for queue in all_queues {
                if present_queue.is_none() && queue.can_present() {
                    present_queue = Some(queue);
                } else {
                    remaining.push(queue);
                }
            }

            (present_queue.unwrap(), remaining)
        };

        // Use remaining queues for the allocator (dedicated queues removed)
        let queue_allocator = dagal::util::queue_allocator::QueueAllocator::from(remaining_queues);
        let surface = surface.unwrap().query_details(physical_device.handle())?;

        Ok((
            Self {
                instance,
                physical_device,
                device,
                allocator,
                present_queue,
                queue_allocator,
            },
            surface,
        ))
    }
}
