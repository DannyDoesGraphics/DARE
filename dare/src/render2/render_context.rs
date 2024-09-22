use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::ptr;
use std::sync::Arc;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use anyhow::Result;
use dagal::ash::vk;
use dagal::traits::AsRaw;
use dagal::winit;
use bevy_ecs::prelude as becs;
use tokio::sync::RwLock;

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

pub struct RenderContextInner {
    pub instance: dagal::core::Instance,
    pub physical_device: dagal::device::PhysicalDevice,
    pub device: dagal::device::LogicalDevice,
    pub allocator: dagal::allocators::ArcAllocator<GPUAllocatorImpl>,
    pub window_context: Arc<super::window_context::WindowContext>,

    pub configuration: RenderContextConfiguration,
}
impl Drop for RenderContextInner {
    fn drop(&mut self) {
        panic!("FUCK");
    }
}

/// Describes the render context
#[derive(Clone, becs::Resource)]
pub struct RenderContext {
    pub inner: Arc<RenderContextInner>,
}


impl RenderContext {
    pub fn new(ci: RenderContextCreateInfo) -> Result<Self> {
        let instance = dagal::bootstrap::InstanceBuilder::new()
            .set_vulkan_version((1, 3, 0));
        #[cfg(debug_assertions)]
        let instance = instance.add_extension(
            dagal::ash::ext::debug_utils::NAME.as_ptr()
        );
        // add required extensions
        let instance = dagal::ash_window::enumerate_required_extensions(ci.rdh)?.into_iter().fold(instance, |mut instance, layer| {
            instance.add_extension(*layer)
        }).build()?;


        // Make physical device
        let physical_device = dagal::bootstrap::PhysicalDeviceSelector::default()
            .add_required_extension(
                dagal::ash::khr::swapchain::NAME.as_ptr()
            )
            .set_minimum_vulkan_version((1,3,0))
            .add_required_queue(dagal::bootstrap::QueueRequest {
                family_flags: vk::QueueFlags::GRAPHICS,
                count: 1,
                dedicated: true,
            })
            .select(&instance)?;
        let physical_device = physical_device.handle;
        // Make logical device
        let device = dagal::bootstrap::LogicalDeviceBuilder::new(physical_device.clone())
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

        // Create allocator
        let allocator = dagal::allocators::ArcAllocator::new(
            dagal::allocators::GPUAllocatorImpl::new(
                gpu_allocator::vulkan::AllocatorCreateDesc {
                    instance: instance.get_instance().clone(),
                    device: device.get_handle().clone(),
                    physical_device: unsafe {
                        *physical_device.as_raw()
                    },
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
                device.clone()
            )?
        );

        // pq
        let present_queue = device.acquire_queue(
            vk::QueueFlags::GRAPHICS,
            Some(true),
            None,
            Some(1)
        )?.pop().unwrap();
        let window_context = Arc::new(super::window_context::WindowContext::new(
            super::window_context::WindowContextCreateInfo {
                present_queue,
                window: None,
            }
        ));

        Ok(Self {
            inner: Arc::new(RenderContextInner {
                instance,
                physical_device,
                device,
                allocator,
                window_context,
                configuration: ci.configuration,
            })
        })
    }

    pub async fn build_surface(&self, window: Arc<winit::window::Window>) -> Result<()> {
        *self.inner.window_context.window.write().await = Some(window.clone());
        self.inner.window_context
            .build_surface(super::surface_context::SurfaceContextCreateInfo {
                instance: & self.inner.instance,
                physical_device: & self.inner.physical_device,
                allocator: self.inner.allocator.clone(),
                window,
                frames_in_flight: Some(self.inner.configuration.target_frames_in_flight),
            }).await?;
        Ok(())
    }
}