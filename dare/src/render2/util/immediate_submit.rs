use std::ptr;
use std::sync::{Arc, LockResult};
use dagal::ash::vk;
use dagal::ash::vk::Handle;
use dagal::command::command_buffer::CmdBuffer;
use dagal::traits::AsRaw;

#[derive(Debug)]
struct ImmediateSubmitInner {
    queue: dagal::device::Queue,
    device: dagal::device::LogicalDevice,
    command_pool: std::sync::Mutex<dagal::command::CommandPool>,
}

/// Immediate submit
#[derive(Debug, Clone)]
pub struct ImmediateSubmit {
    inner: Arc<ImmediateSubmitInner>,
}

impl ImmediateSubmit {
    pub fn new(device: dagal::device::LogicalDevice, queue: dagal::device::Queue) -> anyhow::Result<Self> {
        let command_pool = dagal::command::CommandPool::new(
            device.clone(),
            &queue,
            vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
        )?;
        Ok(Self {
            inner: Arc::new(ImmediateSubmitInner { device, queue, command_pool: std::sync::Mutex::new(command_pool) }),
        })
    }

    pub async fn submit<R, F: FnOnce(&tokio::sync::MutexGuard<vk::Queue>, &dagal::command::CommandBufferRecording) -> R>(&self, func: F) -> anyhow::Result<R> {
        let queue: tokio::sync::MutexGuard<vk::Queue> = self.inner
            .queue
            .acquire_queue_async().await?;
        let command_pool_guard: std::sync::MutexGuard<dagal::command::CommandPool> = match self.inner.command_pool.lock() {
            Ok(pool) => pool,
            Err(e) => {
                tracing::error!("Previous immediate submit failed");
                e.into_inner()
            }
        };
        let command_buffer = command_pool_guard
            .allocate(1)?.pop().unwrap();
        let command_buffer = command_buffer.begin(
            vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT
        ).unwrap();
        let res = func(&queue, &command_buffer);
        let command_buffer = command_buffer.end()?;
        let fence = dagal::sync::Fence::new(
            self.inner.device.clone(),
            vk::FenceCreateFlags::empty()
        )?;
        unsafe {
            self.inner.device
                .get_handle()
                .queue_submit2(*queue, &[vk::SubmitInfo2 {
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
                }], *fence.as_raw())?;
        }
        fence.fence_await()
            .await?;
        anyhow::Ok(res)
    }
}