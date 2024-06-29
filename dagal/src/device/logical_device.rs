use crate::device::physical_device::PhysicalDevice;
use crate::traits::Destructible;
use anyhow::Result;
use ash;
use ash::vk;
use derivative::Derivative;
use std::collections::HashSet;
use std::ffi::c_char;
use std::sync::{Arc, Weak};

#[derive(Clone, Derivative)]
#[derivative(Debug)]
struct LogicalDeviceInner {
    #[derivative(Debug = "ignore")]
    handle: ash::Device,
    /// Contains queue families used
    #[derivative(PartialEq = "ignore")]
    queue_families: Vec<u32>,
    /// Enabled extensions
    enabled_extensions: HashSet<String>,
    /// Debug utils
    #[derivative(PartialEq = "ignore", Debug = "ignore")]
    debug_utils: Option<ash::ext::debug_utils::Device>,
    /// Acceleration structure
    #[derivative(PartialEq = "ignore", Debug = "ignore")]
    acceleration_structure: Option<ash::khr::acceleration_structure::Device>,
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

pub struct LogicalDeviceCreateInfo<'a> {
    pub instance: &'a ash::Instance,
    pub physical_device: PhysicalDevice,
    pub device_ci: vk::DeviceCreateInfo<'a>,
    pub queue_families: Vec<u32>,
    pub enabled_extensions: HashSet<String>,
    pub debug_utils: bool,
}

impl LogicalDevice {
    pub fn new(device_ci: LogicalDeviceCreateInfo) -> Result<Self> {
        let device = unsafe {
            device_ci.instance.create_device(
                *device_ci.physical_device.get_handle(),
                &device_ci.device_ci,
                None,
            )?
        };

        #[cfg(feature = "log-lifetimes")]
        trace!("Creating VkDevice {:p}", device.handle());

        let mut debug_utils: Option<ash::ext::debug_utils::Device> = None;
        if device_ci.debug_utils {
            debug_utils = Some(ash::ext::debug_utils::Device::new(
                device_ci.instance,
                &device,
            ));
        }

        let mut acceleration_structure: Option<ash::khr::acceleration_structure::Device> = None;
        if device_ci.enabled_extensions.contains(
            &crate::util::wrap_c_str(ash::khr::acceleration_structure::NAME.as_ptr())
                .to_string_lossy()
                .to_string(),
        ) {
            acceleration_structure = Some(ash::khr::acceleration_structure::Device::new(
                device_ci.instance,
                &device,
            ));
        }

        Ok(Self {
            inner: Arc::new(LogicalDeviceInner {
                handle: device,
                queue_families: device_ci.queue_families,
                enabled_extensions: device_ci.enabled_extensions,
                debug_utils,
                acceleration_structure,
            }),
        })
    }

    pub fn has_extension(&self, ext: *const c_char) -> bool {
        self.inner
            .enabled_extensions
            .contains(&crate::util::wrap_c_str(ext).to_string_lossy().to_string())
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

    /// Get debug utils with the device
    pub fn get_debug_utils(&self) -> Option<&ash::ext::debug_utils::Device> {
        self.inner.debug_utils.as_ref()
    }

    /// Get the acceleration structure ext
    pub fn get_acceleration_structure(&self) -> Option<&ash::khr::acceleration_structure::Device> {
        self.inner.acceleration_structure.as_ref()
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
