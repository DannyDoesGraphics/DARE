use crate::device::physical_device::PhysicalDevice;
use crate::traits::Destructible;
use anyhow::Result;
use ash;
use ash::vk;
use derivative::Derivative;
use std::sync::{Arc, Weak};

use tracing::trace;

#[derive(Clone, Derivative)]
#[derivative(Debug)]
struct LogicalDeviceInner {
    #[derivative(Debug = "ignore")]
    handle: ash::Device,
    /// Contains queue families used
    #[derivative(PartialEq = "ignore")]
    queue_families: Vec<u32>,
}

impl PartialEq for LogicalDeviceInner {
    fn eq(&self, other: &Self) -> bool {
        self.handle.handle() == other.handle.handle()
    }
}

impl Destructible for LogicalDeviceInner {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        trace!("Destroying VkDevice {:p}", self.handle.handle());

        unsafe {
            self.handle.destroy_device(None);
        }
    }
}

impl Eq for LogicalDeviceInner {}

#[cfg(feature = "raii")]
impl Drop for LogicalDeviceInner {
    fn drop(&mut self) {
        unsafe {
            self.destroy();
        }
    }
}

/// Effectively the same as [`ash::Device`], but will automatically clean itself up if raii is enabled
/// 
/// LogicalDevice encloses [`LogicalDeviceInner`] as it reference counts it using [`Arc`]. This
/// makes lifetime management easier. However, those who opt into deletion stack, may still be
/// able to manually delete the [`LogicalDevice`] using the [`Destructible`] trait.
#[derive(Clone, Derivative, PartialEq, Eq)]
#[derivative(Debug)]
pub struct LogicalDevice {
    #[derivative(Debug = "ignore")]
    inner: Arc<LogicalDeviceInner>,
}

/// A weak reference to a logical device
#[derive(Clone, Derivative)]
pub struct WeakLogicalDevice {
    #[derivative(Debug = "ignore")]
    inner: Weak<LogicalDeviceInner>,
}

impl WeakLogicalDevice {
    /// Upgrades from a [`WeakLogicalDevice`] to a [`LogicalDevice`]
    pub fn upgrade(&self) -> Option<LogicalDevice> {
        self.inner.upgrade().map(|inner| LogicalDevice { inner })
    }
}

impl LogicalDevice {
    pub fn new(
        instance: &ash::Instance,
        physical_device: PhysicalDevice,
        device_ci: &vk::DeviceCreateInfo,
        queue_families: Vec<u32>,
    ) -> Result<Self> {
        let device =
            unsafe { instance.create_device(*physical_device.get_handle(), device_ci, None)? };

        #[cfg(feature = "log-lifetimes")]
        trace!("Creating VkDevice {:p}", device.handle());

        Ok(Self {
            inner: Arc::new(LogicalDeviceInner {
                handle: device,
                queue_families,
            }),
        })
    }

    /// Get reference to the underlying [`ash::Device`]
    pub fn get_handle(&self) -> &ash::Device {
        &self.inner.handle
    }

    /// Acquire a [`vk::Queue`]
    pub fn get_vk_queue(&self, queue_info: &vk::DeviceQueueInfo2) -> vk::Queue {
        unsafe { self.inner.handle.get_device_queue2(queue_info) }
    }

    /// Acquire a [`device::Queue`](crate::device::Queue)
    pub fn get_queue(&self, queue_info: &vk::DeviceQueueInfo2) -> crate::device::Queue {
        let queue = unsafe { self.inner.handle.get_device_queue2(queue_info) };
        crate::device::Queue::new(queue, queue_info.queue_family_index, queue_info.queue_index)
    }

    pub fn get_used_queue_families(&self) -> &[u32] {
        self.inner.queue_families.as_slice()
    }

    /// Downgrades the arc pointer in logical device to allow for garbage collection.
    pub fn downgrade(&self) -> WeakLogicalDevice {
        WeakLogicalDevice {
            inner: Arc::downgrade(&self.inner),
        }
    }
}

impl Destructible for LogicalDevice {
    /// **Safety:** there are zero safety guarantees if the Device is valid. This is realistically
    /// meant to be used to only clean up the logical device and would never be referenced to
    /// after clean up.
    /// 
    /// Possible todo: Replace inner: `Arc<ash::Device>` -> `Arc<Option<ash::Device>>` to enable
    /// safety checking
    fn destroy(&mut self) {
        let device = self.get_handle().clone();
        unsafe {
            device.destroy_device(None);
        }
    }
}

#[cfg(test)]
mod test {}
