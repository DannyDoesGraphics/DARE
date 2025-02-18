use crate::prelude as dare;
use crate::render2::c::CPushConstant;
use crate::render2::surface_context::InnerSurfaceContextCreateInfo;
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
use futures::StreamExt;
use std::ffi::{c_void, CStr, CString};
use std::marker::PhantomData;
use std::mem::{take, ManuallyDrop};
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
        let (instance, physical_device, surface, device, mut allocator, execution_manager) =
            dagal::bootstrap::init::WindowedContext::init(
                dagal::bootstrap::app_info::AppSettings::<winit::window::Window> {
                    name: "DARE".to_string(),
                    version: 0,
                    engine_name: "DARE".to_string(),
                    engine_version: 0,
                    api_version: (1, 3, 0, 0),
                    enable_validation: true,
                    debug_utils: cfg!(debug_assertions),
                    window: Some(&*ci.window),
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
                                queue_type: vec![Expected::Required(vk::QueueFlags::TRANSFER)]
                                    .into(),
                                count: Expected::Preferred(u32::MAX),
                            },
                        ],
                    },
                },
            )?;
        let queue_allocator = dagal::util::queue_allocator::QueueAllocator::from({
            physical_device
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
                .collect::<Vec<dagal::device::Queue>>()
        });
        // pq
        let mut graphics_queue =
            queue_allocator.retrieve_queues(&[], vk::QueueFlags::GRAPHICS, 2)?;
        let queues = graphics_queue
            .iter()
            .map(|queue| (queue.get_index(), queue.get_family_index()))
            .collect::<Vec<(u32, u32)>>();
        let transfer_queues = queue_allocator.retrieve_queues(
            &queues,
            vk::QueueFlags::TRANSFER,
            queue_allocator.matching_queues(&queues, vk::QueueFlags::TRANSFER),
        )?;
        let mut present_queue = graphics_queue.pop().unwrap();
        let immediate_queue = graphics_queue.pop().unwrap();
        let immediate_submit =
            dare::render::util::ImmediateSubmit::new(device.clone(), immediate_queue)?;

        let window_context = super::window_context::WindowContext::new(
            super::window_context::WindowContextCreateInfo {
                present_queue: present_queue.clone(),
                surface: Some(dare::render::contexts::SurfaceContext::new(
                    InnerSurfaceContextCreateInfo {
                        instance: &instance,
                        surface,
                        physical_device: &physical_device,
                        allocator: allocator.clone(),
                        present_queue,
                        window: &ci.window,
                        frames_in_flight: Some(3),
                    },
                )?),
            },
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
                window,
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
