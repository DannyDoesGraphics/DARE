use anyhow::Result;
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::ash::vk::Handle;
use dagal::traits::AsRaw;
use dagal::winit;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::prelude as dare;

pub struct RenderContextCreateInfo {
    pub(crate) rdh: dagal::raw_window_handle::RawDisplayHandle,
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
    pub configuration: RenderContextConfiguration,
    pub window_context: Arc<super::window_context::WindowContext>,

    pub allocator: dagal::allocators::ArcAllocator<GPUAllocatorImpl>,
    pub device: dagal::device::LogicalDevice,
    pub physical_device: dagal::device::PhysicalDevice,
    pub instance: dagal::core::Instance,
}

impl Drop for RenderContextInner {
    fn drop(&mut self) {
        while Arc::strong_count(&self.window_context) >= 2 {}
        unsafe { self.device.get_handle().device_wait_idle().unwrap() }
        tracing::trace!("Dropped RenderContextInner");
    }
}

/// Describes the render context
#[derive(Debug, Clone, becs::Resource)]
pub struct RenderContext {
    pub asset_manager: dare::asset::AssetManager<GPUAllocatorImpl>,
    pub inner: Arc<RenderContextInner>,
}

impl RenderContext {
    pub fn new(ci: RenderContextCreateInfo) -> Result<Self> {
        let instance = dagal::bootstrap::InstanceBuilder::new().set_vulkan_version((1, 3, 0));
        #[cfg(debug_assertions)]
        let instance = instance.add_extension(dagal::ash::ext::debug_utils::NAME.as_ptr());
        // add required extensions
        let instance = dagal::ash_window::enumerate_required_extensions(ci.rdh)?
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
                family_flags: vk::QueueFlags::GRAPHICS,
                count: 1,
                dedicated: true,
            })
            .select(&instance)?;
        // Make logical device
        let (device, queues) =
            dagal::bootstrap::LogicalDeviceBuilder::from(physical_device.clone())
                .add_queue_allocation(dagal::bootstrap::QueueRequest {
                    family_flags: vk::QueueFlags::GRAPHICS,
                    count: 1,
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
                })
                .build(&instance)?;
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
        let mut transfer_queues = queue_allocator
            .retrieve_queues(vk::QueueFlags::TRANSFER, 2)?;
        let present_queue = transfer_queues.pop().unwrap();
        let transfer_queue = transfer_queues.pop().unwrap();

        let window_context = super::window_context::WindowContext::new(
            super::window_context::WindowContextCreateInfo { present_queue },
        );
        let gpu_rt = dare::render::util::GPUResourceTable::<GPUAllocatorImpl>::new(
            device.clone(),
            &mut allocator,
        )?;
        /// 128mb transfers
        let transfer_pool = {
            let transfer_command_pool = dagal::command::CommandPool::new(
                device.clone(),
                &transfer_queue,
                vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER
            )?;
            let transfer_command_pool = Arc::new(transfer_command_pool);
            dare::render::util::TransferPool::new(
                device.clone(),
                vk::DeviceSize::from(128_000_u64),
                allocator.clone(),
                transfer_command_pool,
                Arc::new(vec![transfer_queue]),
            )?
        };

        let asset_manager = {
            use std::any::TypeId;
            dare::asset::AssetManager::new(
                allocator.clone(),
                transfer_pool.clone(),
                gpu_rt,
                vec![
                    TypeId::of::<dare::asset::Buffer<GPUAllocatorImpl>>(),
                    TypeId::of::<dare::asset::Image<GPUAllocatorImpl>>(),
                    TypeId::of::<dare::asset::ImageView<GPUAllocatorImpl>>(),
                ],
                // hold for 10 * fif
                ci.configuration.target_frames_in_flight * 10
            )?
        };

        Ok(Self {
            inner: Arc::new(RenderContextInner {
                instance,
                physical_device,
                device,
                allocator,
                window_context: Arc::new(window_context),
                configuration: ci.configuration,
            }),
            asset_manager,
        })
    }

    pub async fn build_surface(&self, window: &winit::window::Window) -> Result<()> {
        self.inner
            .window_context
            .build_surface(super::surface_context::SurfaceContextCreateInfo {
                instance: &self.inner.instance,
                physical_device: &self.inner.physical_device,
                allocator: self.inner.allocator.clone(),
                window,
                frames_in_flight: Some(self.inner.configuration.target_frames_in_flight),
            })
            .await?;
        Ok(())
    }

    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}
