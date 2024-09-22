use std::collections::HashSet;
use std::ffi::c_char;
use std::sync::{Arc, RwLock, Weak};

use crate::device::physical_device::PhysicalDevice;
use crate::resource::traits::Resource;
use crate::traits::{AsRaw, Destructible};
use crate::DagalError;
use anyhow::Result;
use ash;
use ash::vk;
use derivative::Derivative;

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
    /// Queues
    #[derivative(PartialEq = "ignore")]
    queues: Arc<RwLock<Vec<super::Queue>>>,
}

impl LogicalDeviceInner {
    /// Acquire a [`device::Queue`](crate::device::Queue)
    ///
    /// # Safety
    /// Queues created here do not guarantee thread safety whatsoever with other queues
    pub unsafe fn get_queue(
        &self,
        queue_info: &vk::DeviceQueueInfo2,
        dedicated: bool,
        queue_flags: vk::QueueFlags,
    ) -> crate::device::Queue {
        let queue = unsafe { self.handle.get_device_queue2(queue_info) };
        crate::device::Queue::new(
            queue,
            queue_info.queue_family_index,
            queue_info.queue_index,
            dedicated,
            queue_flags,
        )
    }
}

impl PartialEq for LogicalDeviceInner {
    fn eq(&self, other: &Self) -> bool {
        self.handle.handle() == other.handle.handle()
    }
}

impl Destructible for LogicalDeviceInner {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Destroying VkDevice {:p}", self.handle.handle());

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
    pub queues: Vec<super::Queue>,
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
        tracing::trace!("Creating VkDevice {:p}", device.handle());

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
                queues: Arc::new(RwLock::new(device_ci.queues)),
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

    /// Searches for a queue which matches the queue flags
    /// `get_first_available` parameter acquires the first available queue which doesn't have a lock
    /// on it
    pub fn acquire_queue(
        &self,
        queue_flags: vk::QueueFlags,
        dedicated: Option<bool>,
        get_first_available: Option<bool>,
        count: Option<usize>,
    ) -> Result<Vec<super::Queue>> {
        let mut i: usize = 0;
        Ok(self
            .inner
            .queues
            .read()
            .map_err(|_| DagalError::PoisonError)?
            .iter()
            .filter_map(|queue| {
                if dedicated
                    .map(|dedicated| queue.get_dedicated() == dedicated)
                    .unwrap_or(true)
                    && (queue.get_queue_flags() & queue_flags == queue_flags)
                    && get_first_available
                        .map(|gfa| {
                            let available = queue.get_handle().try_lock().is_ok();
                            available == gfa
                        })
                        .unwrap_or(true)
                    && count.map(|count| i < count).unwrap_or(true)
                {
                    i += 1;
                    Some(queue.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<super::Queue>>())
    }

    /// Acquire the next queue and await until one is not locked
    pub async fn acquire_available_queue(
        &self,
        queue_flags: vk::QueueFlags,
        dedicated: Option<bool>,
        count: Option<usize>,
    ) -> Result<Vec<super::Queue>> {
        loop {
            match self.acquire_queue(queue_flags, dedicated, Some(true), count) {
                Ok(queue) => {
                    if !queue.is_empty() {
                        return Ok(queue);
                    }
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Insert queues
    ///
    /// # Safety
    /// Should only be done at device initialization and no other time
    pub unsafe fn insert_queues(&self, mut queues_in: Vec<super::Queue>) -> Result<()> {
        self.inner
            .queues
            .clone()
            .write()
            .map(|mut queues| queues.append(&mut queues_in))
            .map_err(|_| DagalError::PoisonError)?;
        Ok(())
    }

    /// Acquire a [`device::Queue`](crate::device::Queue)
    ///
    /// # Safety
    /// Queues created here do not guarantee thread safety whatsoever with other queues
    pub unsafe fn get_queue(
        &self,
        queue_info: &vk::DeviceQueueInfo2,
        dedicated: bool,
        queue_flags: vk::QueueFlags,
    ) -> crate::device::Queue {
        let queue = unsafe { self.inner.handle.get_device_queue2(queue_info) };
        crate::device::Queue::new(
            queue,
            queue_info.queue_family_index,
            queue_info.queue_index,
            dedicated,
            queue_flags,
        )
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
