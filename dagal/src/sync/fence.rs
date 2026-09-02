use anyhow::Result;
use ash::vk;
use derivative::Derivative;

use crate::traits::{AsRaw, Destructible};

/// Refer to [Vulkan docs](https://docs.vulkan.org/refpages/latest/refpages/source/VkFence.html).
///
/// # Future await implementation
/// By default, Fences do not poll themselves.
#[derive(Debug, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct Fence {
    handle: vk::Fence,
    device: crate::device::LogicalDevice,
}

impl Fence {
    pub fn new(
        device: crate::device::LogicalDevice,
        flags: vk::FenceCreateFlags,
    ) -> Result<Self, crate::DagalError> {
        let handle = unsafe {
            device
                .get_handle()
                .create_fence(&vk::FenceCreateInfo::default().flags(flags), None)?
        };

        #[cfg(feature = "log-lifetimes")]
        log::trace!("Creating VkFence {:p}", handle);

        Ok(Self { handle, device })
    }

    pub fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }

    /// Waits on the current fence
    /// # Example
    /// ```
    /// use std::time::{Instant, Duration};
    /// use ash::vk;
    /// let ctx = dagal::util::tests::TestHarness::headless().build().unwrap();
    /// // purposely make a fence that waits for a whole second
    /// let fence: dagal::sync::Fence = dagal::sync::Fence::new(ctx.device(), vk::FenceCreateFlags::SIGNALED).unwrap();
    /// unsafe {
    ///     fence.wait(1_000_000_000).unwrap_unchecked(); // wait 1 second (in ns)
    /// }
    /// drop(fence);
    /// ```
    pub fn wait(&self, timeout: u64) -> Result<(), crate::DagalError> {
        unsafe {
            self.device
                .get_handle()
                .wait_for_fences(&[self.handle], true, timeout)?
        }
        Ok(())
    }

    /// Resets the fence
    /// # Example
    /// ```
    /// use std::time::{Instant, Duration};
    /// use ash::vk;
    /// let ctx = dagal::util::tests::TestHarness::headless().build().unwrap();
    /// // purposely make a fence that waits for a whole second
    /// let mut fence: dagal::sync::Fence = dagal::sync::Fence::new(ctx.device(), vk::FenceCreateFlags::SIGNALED).unwrap();
    /// unsafe {
    ///     fence.wait(1_000_000_000).unwrap_unchecked(); // wait 1 second (in ns)
    /// }
    /// fence.reset().unwrap();
    /// drop(fence);
    /// ```
    pub fn reset(&mut self) -> Result<(), crate::DagalError> {
        unsafe { self.device.get_handle().reset_fences(&[self.handle]) }?;
        Ok(())
    }

    /// Get the fence status
    pub fn get_fence_status(&self) -> Result<bool, crate::DagalError> {
        unsafe { Ok(self.device.get_handle().get_fence_status(self.handle)?) }
    }
}

impl std::future::Future for Fence {
    type Output = Result<(), crate::DagalError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.get_fence_status() {
            Ok(true) => std::task::Poll::Ready(Ok(())),
            Ok(false) => std::task::Poll::Pending,
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }
}

impl Destructible for Fence {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        log::trace!("Destroying VkFence {:p}", self.handle);

        unsafe {
            self.device.get_handle().destroy_fence(self.handle, None);
        }
    }
}

impl AsRaw for Fence {
    type RawType = vk::Fence;

    unsafe fn as_raw(&self) -> &Self::RawType {
        &self.handle
    }

    unsafe fn as_raw_mut(&mut self) -> &mut Self::RawType {
        &mut self.handle
    }

    unsafe fn raw(self) -> Self::RawType {
        self.handle
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        self.destroy();
    }
}
