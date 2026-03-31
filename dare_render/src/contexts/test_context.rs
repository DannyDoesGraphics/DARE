use std::marker::PhantomData;
use std::ptr;

use dagal::ash::vk;
use dagal::bootstrap::app_info::Expected;
use dagal::bootstrap::app_info::QueueRequest;
use dagal::bootstrap::init::ContextInit;
use dagal::device::queue::QueueGuardExt;
use dagal::traits::AsRaw;

/// Headless Vulkan instance
#[derive(Debug)]
pub struct TestContext {
    allocator: dagal::allocators::GPUAllocatorImpl,
    queue: dagal::device::Queue,
    device: dagal::device::LogicalDevice,
    physical_device: dagal::device::PhysicalDevice,
    instance: dagal::core::Instance,
}

impl TestContext {
    pub fn new() -> anyhow::Result<Self> {
        let (instance, physical_device, _surface, device, allocator) =
            dagal::bootstrap::init::Context::init(dagal::bootstrap::app_info::AppSettings {
                name: "Test".to_string(),
                version: 0,
                engine_name: "Test".to_string(),
                engine_version: 0,
                api_version: (1, 4, 0, 0),
                enable_validation: true,
                debug_utils: cfg!(debug_assertions),
                raw_display_handle: None,
                raw_window_handle: None,
                surface_format: None,
                present_mode: None,
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
                    device_extensions: vec![Expected::Preferred(
                        dagal::ash::ext::debug_utils::NAME
                            .to_string_lossy()
                            .to_string(),
                    )],
                    queues: vec![QueueRequest {
                        strict: false,
                        queue_type: vec![Expected::Required(
                            vk::QueueFlags::GRAPHICS
                                | vk::QueueFlags::TRANSFER
                                | vk::QueueFlags::COMPUTE,
                        )]
                        .into(),
                        count: Expected::Required(2),
                    }],
                },
            })
            .unwrap();

        // Retrieve transfer queues
        let queue: dagal::device::Queue = physical_device
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
            .collect::<Vec<dagal::device::Queue>>().pop().unwrap();

        Ok(Self {
            instance,
            physical_device,
            device,
            queue,
            allocator,
        })
    }

    /// Get the logical device
    pub fn device(&self) -> dagal::device::LogicalDevice {
        self.device.clone()
    }

    /// Get the queue
    pub fn queue(&self) -> dagal::device::Queue {
        self.queue.clone()
    }

    /// Get the allocator
    pub fn allocator(&self) -> dagal::allocators::GPUAllocatorImpl {
        self.allocator.clone()
    }

    /// Perform immediate submission of GPU commands and await on their completion
    pub async fn immediate_submit<F: FnOnce(&Self, &dagal::command::CommandBufferRecording) -> R, R>(&self, f: F) -> dagal::Result<R> {
        let fence = dagal::sync::Fence::new(self.device.clone(), vk::FenceCreateFlags::empty())?;
        
        let command_pool = dagal::command::CommandPool::new(
            dagal::command::CommandPoolCreateInfo::WithQueue {
                device: self.device.clone(),
                queue: &self.queue,
                flags: vk::CommandPoolCreateFlags::empty(),
            }
        )?;
        let command_buffer = command_pool.allocate(1)?.pop().unwrap();
        let command_buffer = command_buffer.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT).unwrap();
        
        let res = f(&self, &command_buffer);
        
        let command_buffer = command_buffer.end().unwrap();
        
        let submit_infos: Vec<vk::SubmitInfo2> = vec![
            vk::SubmitInfo2 {
                s_type: vk::StructureType::SUBMIT_INFO_2,
                p_next: ptr::null(),
                flags: vk::SubmitFlags::empty(),
                wait_semaphore_info_count: 0,
                p_wait_semaphore_infos: ptr::null(),
                signal_semaphore_info_count: 0,
                p_signal_semaphore_infos: ptr::null(),
                command_buffer_info_count: 1,
                p_command_buffer_infos: &vk::CommandBufferSubmitInfo {
                    s_type: vk::StructureType::COMMAND_BUFFER_SUBMIT_INFO,
                    p_next: ptr::null(),
                    command_buffer: unsafe {
                        *command_buffer.as_raw()
                    },
                    device_mask: 0,
                    _marker: PhantomData::default(),
                },
                _marker: PhantomData::default(),
           }
        ];
        self.queue.acquire_queue_async().await.unwrap().try_submit_async(&self.device, &submit_infos, &fence).await.unwrap();
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dagal::resource::traits::Resource;
    use dagal::command::command_buffer::CmdBuffer;

    /// Literally test if the context can be created
    #[tokio::test]
    async fn test_new() {
        let context = TestContext::new().unwrap();
        drop(context);
    }

    #[tokio::test]
    async fn test_immediate_submit() {
        let mut context = TestContext::new().unwrap();

        // Create a small buffer for testing
        let buffer = dagal::resource::Buffer::new(dagal::resource::BufferCreateInfo::NewEmptyBuffer {
            device: context.device.clone(),
            name: Some("TestBuffer".to_string()),
            allocator: &mut context.allocator,
            size: 64,
            memory_type: dagal::allocators::MemoryLocation::GpuToCpu,
            usage_flags: vk::BufferUsageFlags::TRANSFER_DST,
        }).unwrap();

        // Fill buffer with 0xDEADBEEF pattern using GPU command
        let pattern: u32 = 0xDEADBEEF;
        context.immediate_submit(|_context, recording| {
            unsafe {
                recording.get_device().get_handle().cmd_fill_buffer(
                    *recording.as_raw(),
                    *buffer.as_raw(),
                    0,
                    64,
                    pattern,
                );
            }
        }).await.unwrap();

        // Read back and verify the data
        let data = buffer.read::<u32>(0, 16).unwrap();
        for (i, val) in data.iter().enumerate() {
            assert_eq!(*val, pattern, "Buffer word {} should be 0x{:08X}, got 0x{:08X}", i, pattern, val);
        }
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        unsafe {
            self.device.get_handle().device_wait_idle().unwrap();
        }
    }
}