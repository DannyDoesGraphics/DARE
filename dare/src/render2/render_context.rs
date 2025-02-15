use crate::prelude as dare;
use crate::render2::c::CPushConstant;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::ash::vk::Handle;
use dagal::bootstrap::app_info::{Expected, QueueRequest};
use dagal::bootstrap::init::ContextInit;
use dagal::pipelines::PipelineBuilder;
use dagal::raw_window_handle::HasRawDisplayHandle;
use dagal::traits::AsRaw;
use dagal::winit;
use std::ffi::{c_void, CStr, CString};
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RenderContextCreateInfo {
    pub(crate) window: Arc<winit::window::Window>,
    pub(crate) configuration: RenderContextConfiguration,
}

unsafe impl Send for RenderContextCreateInfo {}

#[derive(Debug, Clone)]
pub struct RenderContextConfiguration {
    pub(crate) target_frames_in_flight: usize,
    pub(crate) target_extent: vk::Extent2D,
}

#[derive(Debug)]
pub struct RenderContextInner {
    pub(super) render_thread: std::sync::RwLock<Option<tokio::task::AbortHandle>>,
    pub(super) configuration: RenderContextConfiguration,
    pub(super) transfer_pool: dare::render::util::TransferPool<GPUAllocatorImpl>,
    pub(super) window_context: Arc<super::window_context::WindowContext>,
    pub(super) new_swapchain_requested: AtomicBool,
    pub(super) graphics_pipeline: dagal::pipelines::GraphicsPipeline,
    pub(super) graphics_layout: dagal::pipelines::PipelineLayout,

    pub(super) immediate_submit: dare::render::util::ImmediateSubmit,
    pub(super) allocator: dagal::allocators::ArcAllocator<GPUAllocatorImpl>,
    pub(super) device: dagal::device::LogicalDevice,
    pub(super) physical_device: dagal::device::PhysicalDevice,
    pub(super) debug_messenger: Option<dagal::device::DebugMessenger>,
    pub(super) instance: dagal::core::Instance,
}

impl Drop for RenderContextInner {
    fn drop(&mut self) {
        while let Some(abort_handle) = self.render_thread.write().unwrap().as_ref() {
            if abort_handle.is_finished() {
                break;
            }
        }
        unsafe { self.device.get_handle().device_wait_idle().unwrap() }
        tracing::trace!("Dropped RenderContextInner");
    }
}

/// Describes the render context
#[derive(Debug, Clone, becs::Resource)]
pub struct RenderContext {
    pub inner: Arc<RenderContextInner>,
}

impl RenderContext {
    pub fn new(ci: RenderContextCreateInfo) -> Result<Self> {
        let mut features_1_3 = vk::PhysicalDeviceVulkan13Features {
            s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_3_FEATURES,
            dynamic_rendering: vk::TRUE,
            synchronization2: vk::TRUE,
            ..Default::default()
        };
        let mut features_1_2 = vk::PhysicalDeviceVulkan12Features {
            s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_2_FEATURES,
            p_next: &mut features_1_3 as *mut _ as *mut c_void,
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
        };
        let mut features_1_1 = vk::PhysicalDeviceVulkan11Features {
            s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_FEATURES,
            p_next: &mut features_1_2 as *mut _ as *mut c_void,
            variable_pointers: vk::TRUE,
            variable_pointers_storage_buffer: vk::TRUE,
            ..Default::default()
        };
        let (instance, physical_device, surface, logical_device, arc_allocator, execution_manager) =
            dagal::bootstrap::init::WindowedContext::init(
                dagal::bootstrap::app_info::AppSettings::<winit::window::Window> {
                    name: "DARE".to_string(),
                    version: 0,
                    engine_name: "DARE".to_string(),
                    engine_version: 0,
                    api_version: (1, 3, 0, 0),
                    enable_validation: false,
                    window: None,
                    surface_format: None,
                    present_mode: None,
                    gpu_requirements: dagal::bootstrap::app_info::GPURequirements {
                        dedicated: Expected::Required(true),
                        features: vk::PhysicalDeviceFeatures2 {
                            s_type: vk::StructureType::PHYSICAL_DEVICE_FEATURES_2,
                            p_next: &mut features_1_1 as *mut _ as *mut c_void,
                            features: vk::PhysicalDeviceFeatures {
                                shader_int64: vk::TRUE,
                                ..Default::default()
                            },
                            _marker: Default::default(),
                        },
                        device_extensions: vec![Expected::Preferred(
                            dagal::ash::ext::debug_utils::NAME
                                .to_string_lossy()
                                .to_string(),
                        )],
                        queues: vec![
                            QueueRequest {
                                strict: false,
                                queue_type: vec![Expected::Required(
                                    vk::QueueFlags::GRAPHICS
                                        | vk::QueueFlags::TRANSFER
                                        | vk::QueueFlags::COMPUTE,
                                )]
                                .into(),
                                count: Expected::Required(1),
                            },
                            QueueRequest {
                                strict: true,
                                queue_type: vec![Expected::Required(vk::QueueFlags::TRANSFER)]
                                    .into(),
                                count: Expected::Preferred(u32::MAX),
                            },
                        ],
                    },
                },
            )?;

        let instance = dagal::bootstrap::InstanceBuilder::new().set_vulkan_version((1, 3, 0));
        let instance = instance
            .add_extension(dagal::ash::ext::debug_utils::NAME.as_ptr())
            .set_validation(cfg!(feature = "tracing"));
        // add required extensions
        let instance = dagal::ash_window::enumerate_required_extensions(unsafe {
            ci.window.raw_display_handle().unwrap()
        })?
        .into_iter()
        .fold(instance, |mut instance, layer| {
            instance.add_extension(*layer)
        })
        .build()?;

        // Make physical device
        let physical_device = dagal::bootstrap::PhysicalDeviceSelector::default()
            .add_required_extension(dagal::ash::khr::swapchain::NAME.as_ptr())
            .set_minimum_vulkan_version((1, 3, 0))
            .add_required_queue(dagal::bootstrap::QueueRequest {
                family_flags: vk::QueueFlags::TRANSFER,
                count: 2,
                dedicated: true,
            })
            .add_required_queue(dagal::bootstrap::QueueRequest {
                family_flags: vk::QueueFlags::GRAPHICS | vk::QueueFlags::TRANSFER,
                count: 2,
                dedicated: true,
            })
            .select(&instance)?;
        // Make logical device
        let device_builder = dagal::bootstrap::LogicalDeviceBuilder::from(physical_device.clone())
            .add_queue_allocation(dagal::bootstrap::QueueRequest {
                family_flags: vk::QueueFlags::GRAPHICS | vk::QueueFlags::TRANSFER,
                count: 2,
                dedicated: true,
            })
            .add_queue_allocation(dagal::bootstrap::QueueRequest {
                family_flags: vk::QueueFlags::TRANSFER,
                count: 2,
                dedicated: true,
            })
            .attach_feature_1_3(vk::PhysicalDeviceVulkan13Features {
                dynamic_rendering: vk::TRUE,
                synchronization2: vk::TRUE,
                ..Default::default()
            })
            .attach_feature_1_2(vk::PhysicalDeviceVulkan12Features {
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
            })
            .attach_feature_1_1(vk::PhysicalDeviceVulkan11Features {
                variable_pointers: vk::TRUE,
                variable_pointers_storage_buffer: vk::TRUE,
                ..Default::default()
            })
            .attach_feature_1_0(vk::PhysicalDeviceFeatures {
                shader_int64: vk::TRUE,
                ..Default::default()
            });
        let device_builder = device_builder.debug_utils(true);

        let (device, queues) = device_builder.build(&instance)?;
        let queue_allocator = dagal::util::queue_allocator::QueueAllocator::from(queues);
        let physical_device: dagal::device::PhysicalDevice = physical_device.into();
        // Create allocator
        let mut allocator = dagal::allocators::ArcAllocator::new(GPUAllocatorImpl::new(
            gpu_allocator::vulkan::AllocatorCreateDesc {
                instance: instance.get_instance().clone(),
                device: device.get_handle().clone(),
                physical_device: unsafe { *physical_device.as_raw() },
                debug_settings: gpu_allocator::AllocatorDebugSettings {
                    log_memory_information: false,
                    log_leaks_on_shutdown: true,
                    store_stack_traces: false,
                    log_allocations: false,
                    log_frees: false,
                    log_stack_traces: false,
                },
                buffer_device_address: true,
                allocation_sizes: Default::default(),
            },
            device.clone(),
        )?);

        // pq
        let transfer_queues = queue_allocator.retrieve_queues(vk::QueueFlags::TRANSFER, 2)?;
        let mut graphics_queue = queue_allocator.retrieve_queues(vk::QueueFlags::GRAPHICS, 2)?;
        let mut present_queue = graphics_queue.pop().unwrap();
        let immediate_queue = graphics_queue.pop().unwrap();
        let immediate_submit =
            dare::render::util::ImmediateSubmit::new(device.clone(), immediate_queue)?;

        let window_context = super::window_context::WindowContext::new(
            super::window_context::WindowContextCreateInfo { present_queue },
        );
        let gpu_rt = dare::render::util::GPUResourceTable::<GPUAllocatorImpl>::new(
            device.clone(),
            &mut allocator,
        )?;
        /// 256kb transfers
        let transfer_pool = {
            dare::render::util::TransferPool::new(
                device.clone(),
                vk::DeviceSize::from(256_000_u64),
                vk::DeviceSize::from(2_256_000_u64),
                transfer_queues,
            )?
        };

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
        let debug_messenger =
            dagal::device::DebugMessenger::new(instance.get_entry(), instance.get_instance())?;

        Ok(Self {
            inner: Arc::new(RenderContextInner {
                render_thread: Default::default(),
                instance,
                physical_device,
                device,
                allocator,
                window_context: Arc::new(window_context),
                configuration: ci.configuration,
                transfer_pool,
                graphics_pipeline,
                graphics_layout: graphics_pipeline_layout,
                debug_messenger: None,
                immediate_submit,
                new_swapchain_requested: AtomicBool::new(false),
            }),
        })
    }

    pub fn update_surface(&self, window: &winit::window::Window) -> Result<()> {
        self.inner.window_context.update_surface(
            super::surface_context::SurfaceContextUpdateInfo {
                instance: &self.inner.instance,
                physical_device: &self.inner.physical_device,
                allocator: self.inner.allocator.clone(),
                window: window,
                frames_in_flight: Some(self.inner.configuration.target_frames_in_flight),
            },
        )?;
        Ok(())
    }

    /// Get a transfer pool copy
    pub fn transfer_pool(&self) -> dare::render::util::TransferPool<GPUAllocatorImpl> {
        self.inner.transfer_pool.clone()
    }

    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}
