/// Command buffers have been divided into 2 structs: [`CommandBuffer`] and [`CommandBufferRecording`].
///
/// This type state ensures that no commands are submitted when they're not supposed to.
/// **Safety:** We do not make guarantees for Invalid command buffers. It is your responsibility to
/// deal with such.
use std::ops::Deref;
use std::ptr;

use anyhow::Result;
use ash::vk;

#[derive(Debug, Clone)]
pub struct CommandBuffer {
    handle: vk::CommandBuffer,
    device: crate::device::LogicalDevice,
}

impl CommandBuffer {
    pub fn new(handle: vk::CommandBuffer, device: crate::device::LogicalDevice) -> Self {
        Self { handle, device }
    }

    /// If a command buffer submission fails for whatever reason, it [`Err`] returns a tuple
    /// containing the original command buffer and the [`VkResult`](vk::Result) error.
    pub fn begin(
        self,
        flags: vk::CommandBufferUsageFlags,
    ) -> Result<CommandBufferRecording, (CommandBuffer, vk::Result)> {
        let cmd_begin = unsafe {
            self.device.get_handle().begin_command_buffer(
                self.handle,
                &vk::CommandBufferBeginInfo {
                    s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
                    p_next: ptr::null(),
                    flags,
                    p_inheritance_info: ptr::null(),
                    _marker: Default::default(),
                },
            )
        };
        if cmd_begin.is_ok() {
            Ok(CommandBufferRecording {
                handle: self.handle,
                device: self.device.clone(),
            })
        } else {
            let result = cmd_begin.unwrap_err();
            Err((self, result))
        }
    }

    /// Resets the current command buffer
    pub fn reset(&self, flags: vk::CommandBufferResetFlags) -> Result<()> {
        unsafe {
            self.device
                .get_handle()
                .reset_command_buffer(self.handle, flags)?
        };
        Ok(())
    }

    /// Wait
    pub fn wait_fences(&self, fences: &[crate::sync::Fence], time_out: u64) -> Result<()> {
        unsafe {
            self.device.get_handle().wait_for_fences(
                fences
                    .iter()
                    .map(|fence| fence.handle())
                    .collect::<Vec<vk::Fence>>()
                    .as_slice(),
                true,
                time_out,
            )?
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CommandBufferRecording {
    handle: vk::CommandBuffer,
    device: crate::device::LogicalDevice,
}

impl CommandBufferRecording {
    /// Create a new [`CommandBufferRecording`] from VkObjects. For internal use only.
    pub(crate) fn from_vk(handle: vk::CommandBuffer, device: crate::device::LogicalDevice) -> Self {
        Self { handle, device }
    }

    /// Ends recording into the command buffer
    pub fn end(self) -> Result<CommandBufferExecutable> {
        unsafe { self.device.get_handle().end_command_buffer(self.handle)? }
        Ok(CommandBufferExecutable {
            handle: self.handle,
            device: self.device,
        })
    }

    /// Acquire a dynamic rendering context from the current [`CommandBufferRecording`]
    pub fn dynamic_rendering(&self) -> crate::command::DynamicRenderContext {
        crate::command::DynamicRenderContext::from_vk(self)
    }

    /// SAFETY: You should never be cloning command buffers around, but this is done to help with utility internally
    pub unsafe fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            device: self.device.clone(),
        }
    }
}

/// Command buffer is in its executable state and can now be executed via queue submission
#[derive(Debug)]
pub struct CommandBufferExecutable {
    handle: vk::CommandBuffer,
    device: crate::device::LogicalDevice,
}

impl CommandBufferExecutable {
    unsafe fn clone(&self) -> Self {
        Self {
            handle: self.handle,
            device: self.device.clone(),
        }
    }

    /// Quickly acquire a [`VkCommandBufferSubmitInfo`](vk::CommandBufferSubmitInfo) for
    /// a single [`VkCommandBuffer`](vk::CommandBuffer).
    pub fn submit_info(&self) -> vk::CommandBufferSubmitInfo<'static> {
        vk::CommandBufferSubmitInfo {
            s_type: vk::StructureType::COMMAND_BUFFER_SUBMIT_INFO,
            p_next: ptr::null(),
            command_buffer: self.handle,
            device_mask: 0,
            _marker: Default::default(),
        }
    }

    /// Submit with synchronization primitives
    pub fn submit_info_sync<'a>(
        cmd_submit_info: &[vk::CommandBufferSubmitInfo<'a>],
        wait_semaphores: &[vk::SemaphoreSubmitInfo<'a>],
        signal_semaphore: &[vk::SemaphoreSubmitInfo<'a>],
    ) -> vk::SubmitInfo2<'a> {
        vk::SubmitInfo2 {
            s_type: vk::StructureType::SUBMIT_INFO_2,
            p_next: ptr::null(),
            flags: vk::SubmitFlags::empty(),
            wait_semaphore_info_count: wait_semaphores.len() as u32,
            p_wait_semaphore_infos: wait_semaphores.as_ptr(),
            command_buffer_info_count: cmd_submit_info.len() as u32,
            p_command_buffer_infos: cmd_submit_info.as_ptr(),
            signal_semaphore_info_count: signal_semaphore.len() as u32,
            p_signal_semaphore_infos: signal_semaphore.as_ptr(),
            _marker: Default::default(),
        }
    }

    /// Submits the current command buffer to the queue
    pub fn submit(
        self,
        queue: vk::Queue,
        submit_infos: &[vk::SubmitInfo2],
        fence: vk::Fence,
    ) -> Result<CommandBuffer, (CommandBuffer, vk::Result)> {
        let res = unsafe {
            self.device
                .get_handle()
                .queue_submit2(queue, submit_infos, fence)
        };
        let cmd_buf = CommandBuffer {
            handle: self.handle,
            device: self.device,
        };
        if res.is_ok() {
            Ok(cmd_buf)
        } else {
            Err((cmd_buf, res.unwrap_err()))
        }
    }
}

/// Traits that all command buffers are expected to have
pub trait CmdBuffer: Deref {
    /// Get the [`VkDevice`](ash::Device) attached
    fn get_device(&self) -> &crate::device::LogicalDevice;
    /// Get the underlying [`VkCommandBuffer`](vk::CommandBuffer) reference
    fn get_handle(&self) -> &vk::CommandBuffer;
    /// Get the underlying [`VkCommandBuffer`](vk::CommandBuffer) copy
    fn handle(&self) -> vk::CommandBuffer;
}

impl CmdBuffer for CommandBufferExecutable {
    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }

    fn get_handle(&self) -> &vk::CommandBuffer {
        &self.handle
    }

    fn handle(&self) -> vk::CommandBuffer {
        self.handle
    }
}

impl Deref for CommandBufferExecutable {
    type Target = vk::CommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl CmdBuffer for CommandBufferRecording {
    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }

    fn get_handle(&self) -> &vk::CommandBuffer {
        &self.handle
    }

    fn handle(&self) -> vk::CommandBuffer {
        self.handle
    }
}

impl Deref for CommandBufferRecording {
    type Target = vk::CommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl CmdBuffer for CommandBuffer {
    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }

    fn get_handle(&self) -> &vk::CommandBuffer {
        &self.handle
    }

    fn handle(&self) -> vk::CommandBuffer {
        self.handle
    }
}

impl Deref for CommandBuffer {
    type Target = vk::CommandBuffer;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

#[derive(Debug)]
pub enum CommandBufferState {
    Ready(CommandBuffer),
    Recording(CommandBufferRecording),
    Executable(CommandBufferExecutable),
}

impl From<CommandBuffer> for CommandBufferState {
    fn from(value: CommandBuffer) -> Self {
        Self::Ready(value)
    }
}

impl From<CommandBufferRecording> for CommandBufferState {
    fn from(value: CommandBufferRecording) -> Self {
        Self::Recording(value)
    }
}

impl From<CommandBufferExecutable> for CommandBufferState {
    fn from(value: CommandBufferExecutable) -> Self {
        Self::Executable(value)
    }
}

impl Deref for CommandBufferState {
    type Target = vk::CommandBuffer;

    fn deref(&self) -> &Self::Target {
        match self {
            CommandBufferState::Ready(r) => r,
            CommandBufferState::Recording(r) => r,
            CommandBufferState::Executable(r) => r,
        }
    }
}

impl CmdBuffer for CommandBufferState {
    fn get_device(&self) -> &crate::device::LogicalDevice {
        match self {
            CommandBufferState::Ready(r) => r.get_device(),
            CommandBufferState::Recording(r) => r.get_device(),
            CommandBufferState::Executable(r) => r.get_device(),
        }
    }

    fn get_handle(&self) -> &vk::CommandBuffer {
        match self {
            CommandBufferState::Ready(r) => r.get_handle(),
            CommandBufferState::Recording(r) => r.get_handle(),
            CommandBufferState::Executable(r) => r.get_handle(),
        }
    }

    fn handle(&self) -> vk::CommandBuffer {
        match self {
            CommandBufferState::Ready(r) => r.handle(),
            CommandBufferState::Recording(r) => r.handle(),
            CommandBufferState::Executable(r) => r.handle(),
        }
    }
}

impl CommandBufferState {
    pub fn begin(&mut self, flags: vk::CommandBufferUsageFlags) -> Result<()> {
        *self = Self::from(match self {
            CommandBufferState::Recording(r) => {
                return Err(anyhow::anyhow!(
                    "Expected command buffer state to be in Ready, got Recording"
                ))
            }
            CommandBufferState::Executable(_) => {
                return Err(anyhow::anyhow!(
                    "Expected command buffer state to be in Ready, got Executable"
                ))
            }
            CommandBufferState::Ready(cmd) => unsafe {
                Ok::<CommandBufferRecording, anyhow::Error>(cmd.clone().begin(flags).unwrap())
            },
        }?);
        Ok(())
    }

    // Recording
    pub fn end(&mut self) -> Result<()> {
        *self = Self::from(match self {
            CommandBufferState::Recording(r) => unsafe {
                Ok::<CommandBufferExecutable, anyhow::Error>(r.clone().end()?)
            },
            CommandBufferState::Executable(_) => {
                return Err(anyhow::anyhow!(
                    "Expected command buffer state to be in Recording, got Executable"
                ))
            }
            CommandBufferState::Ready(_) => {
                return Err(anyhow::anyhow!(
                    "Expected command buffer state to be in Recording, got Ready"
                ))
            }
        }?);
        Ok(())
    }

    // Executable
    pub fn submit(
        &mut self,
        queue: vk::Queue,
        submit_infos: &[vk::SubmitInfo2],
        fence: vk::Fence,
    ) -> Result<()> {
        *self = Self::from(match self {
            CommandBufferState::Executable(r) => unsafe {
                Ok::<CommandBuffer, anyhow::Error>(
                    r.clone().submit(queue, submit_infos, fence).unwrap(),
                )
            },
            CommandBufferState::Recording(_) => {
                return Err(anyhow::anyhow!(
                    "Command buffer state expected to be in Executable, got Recording"
                ))
            }
            CommandBufferState::Ready(_) => {
                return Err(anyhow::anyhow!(
                    "Command buffer state expected to be in Executable, got Ready"
                ))
            }
        }?);
        Ok(())
    }
}
