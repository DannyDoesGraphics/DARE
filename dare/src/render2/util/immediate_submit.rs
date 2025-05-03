use dagal::allocators::GPUAllocatorImpl;
use dagal::ash::vk;
use dagal::ash::vk::Handle;
use dagal::command::command_buffer::CmdBuffer;
use dagal::traits::AsRaw;
use std::ptr;
use std::sync::{Arc, LockResult};

#[derive(Debug)]
struct ImmediateSubmitInner {
    queues: dagal::util::queue_allocator::QueueAllocator<tokio::sync::Mutex<vk::Queue>>,
    device: dagal::device::LogicalDevice,
}

/// Immediate submit
#[derive(Debug, Clone)]
pub struct ImmediateSubmit {
    inner: Arc<ImmediateSubmitInner>,
}

impl ImmediateSubmit {
    pub fn new(
        device: dagal::device::LogicalDevice,
        queues: dagal::util::queue_allocator::QueueAllocator<tokio::sync::Mutex<vk::Queue>>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            inner: Arc::new(ImmediateSubmitInner { queues, device }),
        })
    }

    pub async fn submit<
        R,
        F: FnOnce(&tokio::sync::MutexGuard<vk::Queue>, &dagal::command::CommandBufferRecording) -> R,
    >(
        &self,
        domain: vk::QueueFlags,
        func: F,
    ) -> anyhow::Result<R> {
        let queue: dagal::device::Queue<tokio::sync::Mutex<vk::Queue>> = self
            .inner
            .queues
            .retrieve_queues(None, vk::QueueFlags::TRANSFER, 1)?
            .pop()
            .unwrap();
        let queue_guard: tokio::sync::MutexGuard<vk::Queue> = queue.acquire_queue_async().await?;
        let command_pool = dagal::command::command_pool::CommandPool::new(
            dagal::command::command_pool::CommandPoolCreateInfo::WithQueue {
                device: self.inner.device.clone(),
                queue: &queue,
                flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
            },
        )?;
        let command_buffer = command_pool.allocate(1)?.pop().unwrap();
        let command_buffer = command_buffer
            .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .unwrap();
        let res = func(&queue_guard, &command_buffer);
        let command_buffer = command_buffer.end()?;
        let fence =
            dagal::sync::Fence::new(self.inner.device.clone(), vk::FenceCreateFlags::empty())?;
        unsafe {
            self.inner.device.get_handle().queue_submit2(
                *queue_guard,
                &[vk::SubmitInfo2 {
                    s_type: vk::StructureType::SUBMIT_INFO_2,
                    p_next: ptr::null(),
                    flags: vk::SubmitFlags::empty(),
                    wait_semaphore_info_count: 0,
                    p_wait_semaphore_infos: ptr::null(),
                    command_buffer_info_count: 1,
                    p_command_buffer_infos: &vk::CommandBufferSubmitInfo {
                        s_type: vk::StructureType::COMMAND_BUFFER_SUBMIT_INFO,
                        p_next: ptr::null(),
                        command_buffer: *command_buffer.get_handle(),
                        device_mask: 0,
                        _marker: Default::default(),
                    },
                    signal_semaphore_info_count: 0,
                    p_signal_semaphore_infos: ptr::null(),
                    _marker: Default::default(),
                }],
                *fence.as_raw(),
            )?;
        }
        fence.fence_await().await?;
        drop(fence);
        drop(command_pool);
        anyhow::Ok(res)
    }
}
