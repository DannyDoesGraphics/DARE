use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{ptr, thread};

use anyhow::Result;
use ash::vk;

use crate::traits::Destructible;

#[derive(Debug, PartialEq, Eq)]
pub struct Fence {
    handle: vk::Fence,
    device: crate::device::LogicalDevice,
    /// thanks phobos-rs
    wait_thread_spawned: bool,
}

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

        Ok(Self {
            handle,
            device,
            wait_thread_spawned: false,
        })
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
}

impl Destructible for Fence {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        trace!("Destroying VkFence {:p}", self.handle);

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

impl Future for Fence {
    type Output = Result<()>;

    /// A fence's future can be considered ready if:
    /// - The fence has been signaled
    /// - The fence timed out (u64::MAX)
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let status = unsafe { self.device.get_handle().get_fence_status(self.handle) };
        if status.is_err() {
            return Poll::Ready(Err(anyhow::Error::from(status.unwrap_err())));
        }
        let status: bool = status.unwrap();
        if status {
            self.wait_thread_spawned = false;
            return Poll::Ready(Ok(()));
        } else if !self.wait_thread_spawned {
            let waker = cx.waker().clone();
            self.wait_thread_spawned = true;
            let fence = self.handle;
            let device = self.device.get_handle().clone();
            thread::spawn(move || {
                unsafe {
                    device.wait_for_fences(&[fence], true, u64::MAX).unwrap();
                }
                waker.wake();
            });
        }
        Poll::Pending
    }
}
