use crate::prelude as dare;
use crate::render::c::CPushConstant;
use crate::render::contexts::{
    DeviceContext, GraphicsContext, InnerSurfaceContextCreateInfo, TransferContext, WindowContext,
};
use anyhow::Result;
use dagal::ash::vk;
use dagal::bootstrap::app_info::{Expected, QueueRequest};
use dagal::bootstrap::init::ContextInit;
use dagal::pipelines::PipelineBuilder;
use dagal::traits::AsRaw;
use std::ptr;

pub struct ContextsCreateInfo {
    pub(crate) raw_handles: dare::window::WindowHandles,
    pub(crate) configuration: ContextsConfiguration,
}

unsafe impl Send for ContextsCreateInfo {}

#[derive(Debug, Clone)]
pub struct ContextsConfiguration {
    pub(crate) target_frames_in_flight: usize,
    pub(crate) target_extent: vk::Extent2D,
}

pub struct CreatedContexts {
    pub device_context: DeviceContext,
    pub graphics_context: GraphicsContext,
    pub transfer_context: TransferContext,
    pub window_context: WindowContext,
}

/// Create all the separate contexts that replace the monolithic RenderContext
pub fn create_contexts(ci: ContextsCreateInfo) -> Result<CreatedContexts> {
    let (instance, physical_device, surface, device, allocator) =
        dagal::bootstrap::init::WindowedContext::init(dagal::bootstrap::app_info::AppSettings {
            name: "DARE".to_string(),
            version: 0,
            engine_name: "DARE".to_string(),
            engine_version: 0,
            api_version: (1, 3, 0, 0),
            enable_validation: true,
            debug_utils: cfg!(debug_assertions),
            raw_display_handle: Some(*ci.raw_handles.raw_display_handle),
            raw_window_handle: Some(*ci.raw_handles.raw_window_handle),
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

    let (graphics_queue, present_queue, remaining_queues) = {
        let mut graphics_queue = None;
        let mut present_queue = None;
        let mut remaining = Vec::new();

        for queue in all_queues {
            if graphics_queue.is_none()
                && queue.get_queue_flags().contains(vk::QueueFlags::GRAPHICS)
            {
                graphics_queue = Some(queue);
            } else if present_queue.is_none() && queue.can_present() {
                present_queue = Some(queue);
            } else {
                remaining.push(queue);
            }
        }

        (graphics_queue.unwrap(), present_queue.unwrap(), remaining)
    };

    // Use remaining queues for the allocator (dedicated queues removed)
    let queue_allocator = dagal::util::queue_allocator::QueueAllocator::from(remaining_queues);

    // Create window context first (needs to borrow instance and physical_device)
    let window_context = WindowContext::new(crate::render::contexts::WindowContextCreateInfo {
        present_queue: present_queue.clone(),
        surface: Some(dare::render::contexts::SurfaceContext::new(
            InnerSurfaceContextCreateInfo {
                instance: &instance,
                surface,
                physical_device: &physical_device,
                allocator: allocator.clone(),
                present_queue,
                raw_handles: ci.raw_handles.clone(),
                extent: (0, 0),
                frames_in_flight: Some(4),
            },
        )?),
        window_handles: ci.raw_handles.clone(),
    });

    // Create device context (takes ownership of instance and physical_device)
    let device_context = DeviceContext::new(
        instance,
        physical_device,
        device.clone(),
        allocator.clone(),
        None, // debug_messenger
    );

    let immediate_submit =
        dare::render::util::ImmediateSubmit::new(device.clone(), queue_allocator.clone())?;

    // 256kb transfers
    let transfer_pool = {
        dare::render::util::TransferPool::new(
            device.clone(),
            vk::DeviceSize::from(256_000_u64),
            vk::DeviceSize::from(2_256_000_u64),
            queue_allocator.retrieve_queues(None, vk::QueueFlags::TRANSFER, None)?,
        )?
    };

    let transfer_context = TransferContext::new(transfer_pool, immediate_submit);

    let graphics_pipeline_layout = dagal::pipelines::PipelineLayoutBuilder::default()
        .push_push_constant_struct::<CPushConstant>(vk::ShaderStageFlags::VERTEX)
        .build(device.clone(), vk::PipelineLayoutCreateFlags::empty())?;
    let graphics_pipeline = dagal::pipelines::GraphicsPipelineBuilder::default()
        .replace_layout(unsafe { *graphics_pipeline_layout.as_raw() })
        .set_input_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
        .set_polygon_mode(vk::PolygonMode::FILL)
        .set_cull_mode(vk::CullModeFlags::NONE, vk::FrontFace::CLOCKWISE)
        .set_multisampling_none()
        .enable_blending_alpha_blend()
        .enable_depth_test(vk::TRUE, vk::CompareOp::GREATER_OR_EQUAL)
        .set_depth_format(vk::Format::D32_SFLOAT)
        .set_color_attachment(vk::Format::R16G16B16A16_SFLOAT)
        .replace_shader_from_spirv_file(
            device.clone(),
            std::path::PathBuf::from("./dare/shaders/compiled/solid.vert.spv"),
            vk::ShaderStageFlags::VERTEX,
        )
        .unwrap()
        .replace_shader_from_spirv_file(
            device.clone(),
            std::path::PathBuf::from("./dare/shaders/compiled/solid.frag.spv"),
            vk::ShaderStageFlags::FRAGMENT,
        )
        .unwrap()
        .build(device.clone())?;
    let graphics_context = GraphicsContext::new(graphics_pipeline, graphics_pipeline_layout);

    let cull_descriptor_pool = unsafe {
        device.get_handle().create_descriptor_pool(
            &vk::DescriptorPoolCreateInfo {
                s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::DescriptorPoolCreateFlags::empty(),
                max_sets: 1,
                pool_size_count: 1,
                p_pool_sizes: &vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::STORAGE_BUFFER,
                    descriptor_count: 1,
                },
                _marker: std::marker::PhantomData,
            },
            None,
        )
    }?;

    let cull_descriptor_set_layout: vk::DescriptorSetLayout = unsafe {
        device.get_handle().create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo {
                s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::DescriptorSetLayoutCreateFlags::empty(),
                binding_count: 1,
                p_bindings: &vk::DescriptorSetLayoutBinding {
                    binding: 0,
                    descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
                    descriptor_count: 1,
                    stage_flags: vk::ShaderStageFlags::COMPUTE,
                    p_immutable_samplers: ptr::null(),
                    _marker: std::marker::PhantomData::default(),
                },
                _marker: std::marker::PhantomData,
            },
            None,
        )
    }?;
    let cull_descriptor_sets = unsafe {
        device
            .get_handle()
            .allocate_descriptor_sets(&vk::DescriptorSetAllocateInfo {
                s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
                p_next: ptr::null(),
                descriptor_pool: cull_descriptor_pool,
                descriptor_set_count: 1,
                p_set_layouts: &cull_descriptor_set_layout,
                ..Default::default()
            })?
    };

    /*
    let compute_pipeline_layout = dagal::pipelines::PipelineLayoutBuilder::default()
        .push
    */
    Ok(CreatedContexts {
        device_context,
        graphics_context,
        transfer_context,
        window_context,
    })
}
