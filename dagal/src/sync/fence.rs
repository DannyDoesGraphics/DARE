use std::pin::Pin;
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
    pub fn reset(&mut self) -> Result<()> {
        unsafe { self.device.get_handle().reset_fences(&[self.handle]) }?;
        Ok(())
    }

    /// Get the fence status
    pub fn get_fence_status(&self) -> Result<bool, crate::DagalError> {
        unsafe { Ok(self.device.get_handle().get_fence_status(self.handle)?) }
    }

    /// Get a struct which can await on a fence
    pub fn fence_await(&self) -> FenceWait {
        FenceWait { fence: self, waiters: Arc::new(AtomicWaker::new()), thread: Arc::new(Mutex::new(None)) }
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

/// Defines a struct which awaits on a fence
pub struct FenceWait<'a> {
    pub fence: &'a Fence,
    thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    waiters: Arc<AtomicWaker>,
}
impl<'a> Future for FenceWait<'a> {
    type Output = Result<(), crate::DagalError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Fast path: already signaled?
        match self.fence.get_fence_status() {
            Ok(true) => {
                return Poll::Ready(Ok(()))
            }
            Ok(false) => {}
            Err(e) => return Poll::Ready(Err(e)),
        }

        self.waiters.register(cx.waker());

        // Opportunistically reclaim finished waiter so we can spawn a new cycle immediately
        {
            let mut th = match self.thread.lock() {
                Ok(t) => t,
                Err(p) => {
                    let t = p.into_inner();
                    self.thread.clear_poison();
                    t
                }
            };

            if let Some(h) = th.as_ref() {
                if h.is_finished() {
                    if let Some(h) = th.take() {
                        let _ = h.join();
                    }
                }
            }

            // Spawn exactly one blocking waiter for this fence
            if th.is_none() {
                let device = self.fence.device.clone();
                let raw_fence = self.fence.handle;
                let waiters = self.waiters.clone();
                let thread_slot: Arc<Mutex<Option<std::thread::JoinHandle<()>>>> = self.thread.clone();

                let j = std::thread::spawn(move || {
                    let _ = unsafe {
                        device
                            .get_handle()
                            .wait_for_fences(&[raw_fence], true, u64::MAX)
                    };
                    waiters.wake();
                    if let Ok(mut s) = thread_slot.lock() {
                        *s = None;
                    } else {
                        thread_slot.clear_poison();
                        *thread_slot.lock().unwrap() = None;
                    }
                });
                *th = Some(j);
            }
        }

        Poll::Pending
    }
}
