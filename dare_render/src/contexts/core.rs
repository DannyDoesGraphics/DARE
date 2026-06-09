use dagal::allocators::GPUAllocatorImpl;

use dare_window::WindowHandles;

#[derive(Debug)]
pub struct CoreContext {
    pub queues: dagal::device::QueueRegistry,
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
            dagal::bootstrap::init::Context::init(dagal::bootstrap::app_info::AppSettings {
                name: "DARE".to_string(),
                version: 0,
                engine_name: "DARE".to_string(),
                engine_version: 0,
                api_version: (1, 4, 0, 0),
                enable_validation: true,
                debug_utils: false,
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
                            queue_type: vec![Expected::Required(vk::QueueFlags::TRANSFER)].into(),
                            count: Expected::Preferred(u32::MAX),
                        },
                    ],
                },
            })?;

        let queues = dagal::device::QueueRegistry::from_device(&device, &physical_device)?;
        let surface = surface.unwrap().query_details(physical_device.handle())?;

        Ok((
            Self {
                queues,
                allocator,
                device,
                physical_device,
                instance,
            },
            surface,
        ))
    }
}
