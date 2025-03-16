use std::future::Future;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll};

use anyhow::Result;
use ash::vk;

use crate::traits::{AsRaw, Destructible};

#[derive(Debug, PartialEq, Eq)]
pub struct Fence {
    handle: vk::Fence,
    device: crate::device::LogicalDevice,
}
impl Unpin for Fence {}
impl Fence {
    pub fn new(device: crate::device::LogicalDevice, flags: vk::FenceCreateFlags) -> Result<Self> {
        let handle = unsafe {
            device.get_handle().create_fence(
                &vk::FenceCreateInfo {
                    s_type: vk::StructureType::FENCE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags,
                    _marker: Default::default(),
                },
                None,
            )?
        };

        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Creating VkFence {:p}", handle);

        Ok(Self { handle, device })
    }

    /// Gets underlying reference of the handle
    pub fn get_handle(&self) -> &vk::Fence {
        &self.handle
    }

    /// Gets underlying copy of the handle
    pub fn handle(&self) -> vk::Fence {
        self.handle
    }

    /// Waits on the current fence
    /// # Example
    /// ```
    /// use std::time::{Instant, Duration};
    /// use ash::vk;
    /// use dagal::util::tests::TestSettings;
    /// let test_vulkan = dagal::util::tests::create_vulkan_and_device(TestSettings::default());
    /// // purposely make a fence that waits for a whole second
    /// let fence: dagal::sync::Fence = dagal::sync::Fence::new(test_vulkan.device.as_ref().unwrap().clone(), vk::FenceCreateFlags::SIGNALED).unwrap();
    /// unsafe {
    ///     fence.wait(1_000_000_000).unwrap_unchecked(); // wait 1 second (in ns)
    /// }
    /// drop(fence);
    /// ```
    pub fn wait(&self, timeout: u64) -> Result<()> {
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
    /// use dagal::util::tests::TestSettings;
    /// let test_vulkan = dagal::util::tests::create_vulkan_and_device(TestSettings::default());
    /// // purposely make a fence that waits for a whole second
    /// let fence: dagal::sync::Fence = dagal::sync::Fence::new(test_vulkan.device.as_ref().unwrap().clone(), vk::FenceCreateFlags::SIGNALED).unwrap();
    /// unsafe {
    ///     fence.wait(1_000_000_000).unwrap_unchecked(); // wait 1 second (in ns)
    /// }
    /// fence.reset().unwrap();
    /// drop(fence);
    /// ```
    pub fn reset(&self) -> Result<()> {
        unsafe { self.device.get_handle().reset_fences(&[self.handle])? }
        Ok(())
    }

    /// Get the fence status
    pub fn get_fence_status(&self) -> Result<bool> {
        unsafe { Ok(self.device.get_handle().get_fence_status(self.handle)?) }
    }

    /// Get a struct which can await on a fence
    pub fn fence_await(&self) -> FenceWait {
        FenceWait { fence: self }
    }
}

impl Destructible for Fence {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Destroying VkFence {:p}", self.handle);

        unsafe {
            self.device.get_handle().destroy_fence(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl Drop for Fence {
    fn drop(&mut self) {
        self.destroy();
    }
}

/// Defines a struct which awaits on a fence
pub struct FenceWait<'a> {
    pub fence: &'a Fence,
}
impl<'a> Future for FenceWait<'a> {
    type Output = Result<()>;

    /// A fence's future can be considered ready if:
    /// - The fence has been signaled
    /// - The fence timed out (u64::MAX)
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.fence.get_fence_status() {
            Ok(status) => match status {
                true => Poll::Ready(Ok(())),
                false => {
                    let waker = cx.waker().clone();
                    waker.wake_by_ref();
                    Poll::Pending
                }
            },
            Err(e) => Poll::Ready(Err(e)),
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
