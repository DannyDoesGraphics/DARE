use std::{pin::Pin, sync::atomic::{AtomicBool, Ordering}};
use std::ptr;
use std::task::{Context, Poll};
use std::{
    future::Future,
    sync::{Arc, Mutex},
    task::Waker,
};

use anyhow::Result;
use ash::vk;
use derivative::Derivative;
use futures_util::task::AtomicWaker;

use crate::traits::{AsRaw, Destructible};

#[derive(Debug, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct Fence {
    handle: vk::Fence,
    device: crate::device::LogicalDevice,
}
impl Unpin for Fence {}
impl Fence {
    pub fn new(device: crate::device::LogicalDevice, flags: vk::FenceCreateFlags) -> Result<Self, crate::DagalError> {
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
        })
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
    pub fn reset(&mut self) -> Result<(), crate::DagalError> {
        unsafe { self.device.get_handle().reset_fences(&[self.handle]) }?;
        Ok(())
    }

    /// Get the fence status
    pub fn get_fence_status(&self) -> Result<bool, crate::DagalError> {
        unsafe { Ok(self.device.get_handle().get_fence_status(self.handle)?) }
    }

    /// Get a struct which can await on a fence
    pub fn fence_await<'a>(&'a self) -> FenceWait<'a> {
        FenceWait {
            fence: self,
            spawned: AtomicBool::new(false),
            state: Arc::new(WaitState {
                done: AtomicBool::new(false),
                waker: AtomicWaker::new()
            })
        }
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

#[cfg(feature = "raii")]
impl Drop for Fence {
    fn drop(&mut self) {
        self.destroy();
    }
}

struct WaitState {
    done: AtomicBool,
    waker: AtomicWaker,
}

/// Defines a struct which awaits on a fence
pub struct FenceWait<'a> {
    pub fence: &'a Fence,
    state: Arc<WaitState>,
    spawned: AtomicBool,
}
impl<'a> FenceWait<'a> {
    fn spawn_once(&self) {
        if self
            .spawned
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok() {
                let device = self.fence.device.clone();
                let handle = self.fence.handle;
                let state = self.state.clone();
                std::thread::spawn(move || {
                    let _ = unsafe {
                        device.get_handle().wait_for_fences(&[handle], true, u64::MAX)
                    };
                    state.done.store(true, Ordering::SeqCst);
                    state.waker.wake();
                });
            }
    }
}

impl<'a> Future for FenceWait<'a> {
    type Output = Result<(), crate::DagalError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.state.done.load(Ordering::SeqCst) {
            match unsafe { self.fence.device.get_handle().get_fence_status(self.fence.handle) } {
                Ok(true) => return Poll::Ready(Ok(())),
                Err(e) => return Poll::Ready(Err(crate::DagalError::VkError(e))),
                _ => {}
            }
        }
        self.state.waker.register(cx.waker());
        self.spawn_once();
        Poll::Pending
    }
}
