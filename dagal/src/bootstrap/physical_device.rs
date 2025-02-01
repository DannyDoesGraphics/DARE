use std::collections::{HashSet, VecDeque};
use std::ffi::{c_char, CString};
use std::ops::Deref;
use std::ptr;

use anyhow::Result;
use ash::vk;
use derivative::Derivative;

use crate::util::wrap_c_str;

/// Bootstrap for [`PhysicalDevice`](crate::device::PhysicalDevice) used to help initialize a [Device](crate::device::PhysicalDevice)
#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct PhysicalDevice {
    /// Handle to [`crate::device::PhysicalDevice`]
    pub handle: crate::device::PhysicalDevice,

    /// Queues that will be allocated if used to make a [`LogicalDevice`](crate::device::LogicalDevice)
    pub(crate) queues_allocated: Vec<Vec<QueueAllocation>>,

    /// Extensions enabled
    pub extensions_enabled: HashSet<CString>,

    /// Contains the original queue requests made
    pub queue_requests: Vec<crate::bootstrap::QueueRequest>,
}

impl PhysicalDevice {
    /// Discard references and lifetimes to the original queue requests.
    ///
    /// Use this if you are only planning on using [`bootstrap::PhysicalDeviceSelector`](PhysicalDeviceSelector) only.
    pub fn discard_queue_requests(self) -> Self {
        Self {
            handle: self.handle,
            queues_allocated: self.queues_allocated,
            extensions_enabled: self.extensions_enabled,
            queue_requests: vec![],
        }
    }

    /// Get all extensions on the physical device
    pub fn get_extensions(&self) -> &HashSet<CString> {
        &self.extensions_enabled
    }

    pub fn into(self) -> crate::device::PhysicalDevice {
        self.handle
    }
}

impl Deref for PhysicalDevice {
    type Target = crate::device::PhysicalDevice;

    /// Dereferences to the underlying handle
    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

/// Effectively a builder, but to select a suitable physical device
/// # Example
/// ```
/// use ash::vk;
/// use anyhow::Result;
///
/// let test_vulkan = dagal::util::tests::create_vulkan(Default::default());
/// let mut queue_request = vec![
///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true),
///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::GRAPHICS, 1, true)
/// ];
///
/// let physical_device: Result<dagal::bootstrap::PhysicalDevice> = dagal::bootstrap::PhysicalDeviceSelector::default()
/// .add_required_queue(queue_request[0].clone())
/// .add_required_queue(queue_request[1].clone())
/// .set_dedicated(true)
/// .select(test_vulkan.instance.get_instance());
///
/// assert!(physical_device.is_ok());
/// ```
#[derive(Derivative)]
#[derivative(Default)]
pub struct PhysicalDeviceSelector {
    /// If the device is dedicated or not
    #[derivative(Default(value = "None"))]
    dedicated: Option<bool>,

    /// Minimum supported Vulkan version
    #[derivative(Default(value = "(1,0,0)"))]
    min_vulkan_version: (u16, u16, u16),

    /// Requested queues
    required_queues: Vec<crate::bootstrap::QueueRequest>,

    /// Preferred queues
    preferred_queues: Vec<crate::bootstrap::QueueRequest>,

    /// Required extensions
    required_extension: HashSet<CString>,

    /// Preferred extensions
    preferred_extensions: HashSet<CString>,
}

/// Indicates the index + count + queue family index a soon-to-be queue has been allocated for
#[derive(Debug, Clone, PartialOrd, PartialEq)]
pub struct QueueAllocation {
    /// Family index of the queue
    pub family_index: u32,
    /// Index of the queue inside the family
    pub index: u32,
    /// Number of queues
    pub count: u32,
    /// Family flags
    pub family_flags: vk::QueueFlags,
}

/// Get Vulkan version from the version given by Khronos
fn get_version(version: u32) -> (u16, u16, u16) {
    (
        (version >> 22) as u16,
        ((version >> 12) & 0x3ff) as u16,
        (version & 0xfff) as u16,
    )
}

impl PhysicalDeviceSelector {
    /// Set the minimum supported Vulkan version that is deemed suitable
    ///
    /// # Examples
    /// Get devices with at least Vulkan 1.0
    /// ```
    /// use anyhow::Result;
    ///
    /// let test_vulkan = dagal::util::tests::create_vulkan(Default::default()); // Quickly make vulkan
    /// let devices: Result<Vec<dagal::bootstrap::PhysicalDevice>> = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .set_minimum_vulkan_version((1, 0, 0))
    /// .select_all(&test_vulkan.instance);
    /// assert!(devices.is_ok());
    /// assert!(!devices.unwrap().is_empty());
    /// ```
    ///
    ///
    /// Test for a non-existent version of Vulkan
    /// ```
    /// let (test_device) = dagal::util::tests::create_vulkan(Default::default()); // Quickly make vulkan
    /// let devices = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .set_minimum_vulkan_version((u16::MAX, 0, 0))
    /// .select_all(&test_device.instance);
    /// assert!(devices.is_ok());
    /// assert!(devices.unwrap().is_empty()); // At least one Vulkan supported device
    /// ```
    pub fn set_minimum_vulkan_version(mut self, version: (u16, u16, u16)) -> Self {
        self.min_vulkan_version = version;
        self
    }

    /// Set if a suitable physical device must be dedicated
    ///
    /// # Examples
    /// Get physical devices that are dedicated GPUs
    /// ```
    /// let test_vulkan = dagal::util::tests::create_vulkan(Default::default()); // Quickly make vulkan
    /// let devices = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .set_dedicated(true)
    /// .select_all(&test_vulkan.instance);
    /// assert!(devices.is_ok());
    /// println!("There are {} devices that are dedicated.", devices.unwrap().len());
    /// ```
    pub fn set_dedicated(mut self, dedicated: bool) -> Self {
        self.dedicated = Some(dedicated);
        self
    }

    /// Does not care whether the physical device is dedicated
    ///
    /// # Examples
    /// Indicate we wish for dedicated devices, but then state we do not care for them.
    /// In other words, we're looking for any Vulkan device
    /// ```
    /// let test_vulkan = dagal::util::tests::create_vulkan(Default::default()); // Quickly make vulkan
    /// let devices = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .set_dedicated(true)
    /// .dont_care_dedicated()
    /// .select_all(&test_vulkan.instance);
    /// assert!(devices.is_ok());
    /// assert!(!devices.unwrap().is_empty());
    /// ```
    pub fn dont_care_dedicated(mut self) -> Self {
        self.dedicated = None;
        self
    }

    /// Adds a required extension
    ///
    /// # Examples
    /// Select devices which support `VK_KHR_buffer_device_address` at a minimum
    /// ```
    /// use dagal::util::wrap_c_str;
    /// let test_vulkan = dagal::util::tests::create_vulkan(Default::default()); // Quickly make vulkan
    /// let devices = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_required_extension(ash::khr::buffer_device_address::NAME.as_ptr())
    /// .select_all(&test_vulkan.instance);
    /// assert!(devices.is_ok());
    /// let devices = devices.unwrap();
    /// assert!(!devices.is_empty());
    /// let device = devices.get(0).unwrap();
    /// assert!(device.extensions_enabled.contains( &wrap_c_str(ash::khr::buffer_device_address::NAME.as_ptr()) ) );
    /// ```
    pub fn add_required_extension(mut self, extension: *const c_char) -> Self {
        self.required_extension.insert(wrap_c_str(extension));
        self
    }

    /// Adds a preferred extension. If a device has them all, it will be placed first
    ///
    /// # Examples
    /// Prefer `VK_KHR_ray_tracing_pipeline`
    /// ```
    /// use std::ffi::CStr;
    /// use dagal::util::wrap_c_str;
    /// let test_vulkan = dagal::util::tests::create_vulkan(Default::default()); // Quickly make vulkan
    /// let devices = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_preferred_extension(ash::khr::ray_tracing_pipeline::NAME.as_ptr())
    /// .select_all(&test_vulkan.instance);
    /// assert!(devices.is_ok());
    /// assert!(!devices.as_ref().unwrap().is_empty());
    /// assert!(devices.unwrap()[0].extensions_enabled.contains( &wrap_c_str(ash::khr::ray_tracing_pipeline::NAME.as_ptr()) ));
    /// ```
    pub fn add_preferred_extension(mut self, extension: *const c_char) -> Self {
        self.preferred_extensions.insert(wrap_c_str(extension));
        self
    }

    /// Requires a physical device must have this queue
    /// # Examples
    /// Add a requirement for at least 1 graphics queue, 1 compute queue, and 1 transfer queue
    /// ```
    /// use anyhow::Result;
    /// use ash::vk;
    /// let queues = vec![
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true),
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::GRAPHICS, 1, true),
    /// 	dagal::bootstrap::QueueRequest::new(vk::QueueFlags::TRANSFER, 1, true),
    /// ];
    /// let test_vulkan = dagal::util::tests::create_vulkan(Default::default()); // Quickly make vulkan
    /// let devices: Result<Vec<dagal::bootstrap::PhysicalDevice>> = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_required_queue(queues[0].clone())
    /// .add_required_queue(queues[1].clone())
    /// .add_required_queue(queues[2].clone())
    /// .select_all(&test_vulkan.instance);
    /// assert!(devices.is_ok());
    /// assert!(!devices.unwrap().is_empty());
    /// ```
    ///
    /// Test for an impossible number of queues
    /// ```
    /// use anyhow::Result;
    /// use ash::vk;
    /// let queues = vec![
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, u32::MAX, true)
    /// ];
    /// let test_vulkan = dagal::util::tests::create_vulkan(Default::default()); // Quickly make vulkan
    /// let devices: Result<Vec<dagal::bootstrap::PhysicalDevice>> = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_required_queue(queues[0].clone())
    /// .select_all(&test_vulkan.instance);
    /// assert!(devices.is_ok());
    /// assert!(devices.unwrap().is_empty());
    /// ```
    ///
    pub fn add_required_queue(mut self, queue: crate::bootstrap::QueueRequest) -> Self {
        self.required_queues.push(queue);
        self
    }

    /// Adds a preference for a queue, but not a requirement
    /// # Examples
    /// ```
    /// use anyhow::Result;
    /// use ash::vk;
    /// let queues = vec![
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true),
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::TRANSFER, 2, true),
    /// ];
    /// let test_vulkan = dagal::util::tests::create_vulkan(Default::default()); // Quickly make vulkan
    /// let devices: Result<Vec<dagal::bootstrap::PhysicalDevice>> = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_required_queue(queues[0].clone())
    /// .add_preferred_queue(queues[1].clone())
    /// .select_all(&test_vulkan.instance);
    /// assert!(devices.is_ok());
    /// let devices  = devices.unwrap();
    /// assert!(devices.len() > 1);
    /// assert!(devices[0].queue_requests.clone().iter().any(|queue| queue.borrow().count == 1) );
    /// assert!(devices[0].queue_requests.clone().iter().any(|queue| queue.borrow().count == 2) );
    /// drop(devices)
    /// ```
    pub fn add_preferred_queue(mut self, queue: crate::bootstrap::QueueRequest) -> Self {
        self.preferred_queues.push(queue);
        self
    }

    /// Selects all possible suitable physical devices
    /// # Returns
    /// A vector which is ordered in preference/device score (i.e. a device that meets all preferences
    /// will be placed first in the vector while ones that do not but meet minimum requirements are placed back)
    pub fn select_all(mut self, instance: &ash::Instance) -> Result<Vec<PhysicalDevice>> {
        let physical_devices = unsafe { instance.enumerate_physical_devices()? };
        let mut suitable_devices = VecDeque::new();

        for physical_device in physical_devices.into_iter() {
            let queue_families =
                unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
            let mut properties_2 = vk::PhysicalDeviceProperties2 {
                s_type: vk::StructureType::PHYSICAL_DEVICE_PROPERTIES_2,
                p_next: ptr::null_mut(),
                properties: Default::default(),
                _marker: Default::default(),
            };
            unsafe {
                instance.get_physical_device_properties2(physical_device, &mut properties_2);
            };
            let properties = properties_2.properties;
            let extension_names =
                unsafe { instance.enumerate_device_extension_properties(physical_device)? };
            let extension_names: HashSet<CString> = extension_names
                .into_iter()
                .map(|ext| wrap_c_str(ext.extension_name.as_ptr()))
                .collect();
            let mut bs_physical_device = PhysicalDevice {
                handle: crate::device::PhysicalDevice::new(instance, physical_device, Vec::new(), Vec::new()),
                queues_allocated: vec![],
                extensions_enabled: HashSet::new(),
                queue_requests: vec![],
            };
            let mut preferred: bool = true; // Whether the device is preferred and should be pushed to the front

            if self.dedicated.is_some()
                && (properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU)
                    != self.dedicated.unwrap()
            {
                continue;
            }
            {
                // Check versions
                let (min_major, min_minor, min_patch) = self.min_vulkan_version;
                let (major, minor, patch) = get_version(properties.api_version);
                if min_major > major {
                    continue;
                } else if major > min_major && min_minor > minor {
                    continue;
                } else if major > min_major && minor > min_minor && min_patch > patch {
                    continue;
                }
            }
            // determine if it fits bare minimum
            let slotting = crate::bootstrap::queue::determine_queue_slotting(
                queue_families.clone(),
                self.required_queues.clone(),
            );
            if slotting.is_err() {
                continue;
            }
            let has_required_extensions = self
                .required_extension
                .iter()
                .all(|ext| extension_names.contains(ext));
            if !has_required_extensions {
                continue;
            }
            // Consider preferred if possible
            if !self.preferred_queues.is_empty() {
                let mut preferred_queues = self.preferred_queues.clone();
                preferred_queues.append(&mut self.required_queues.clone());
                let slotting_preference = crate::bootstrap::queue::determine_queue_slotting(
                    queue_families.clone(),
                    preferred_queues.clone(),
                );
                if let Ok(slotting_preference) = slotting_preference {
                    bs_physical_device.queues_allocated = slotting_preference;
                    self.required_queues = preferred_queues;
                } else if let Ok(slotting) = slotting {
                    bs_physical_device.queues_allocated = slotting;
                    preferred = false;
                }
            }
            if !self.preferred_extensions.is_empty() {
                if self
                    .preferred_extensions
                    .iter()
                    .all(|ext| extension_names.contains(ext))
                {
                    self.required_extension
                        .extend(self.preferred_extensions.clone());
                } else {
                    preferred = false
                }
            }
            bs_physical_device
                .extensions_enabled
                .clone_from(&self.required_extension); // no fucking clue why i need to clone
            bs_physical_device
                .queue_requests
                .clone_from(&self.required_queues);
            // put physical device into suitable devices
            if preferred {
                suitable_devices.push_front(bs_physical_device);
            } else {
                suitable_devices.push_back(bs_physical_device);
            }
        }
        Ok(Vec::from(suitable_devices))
    }

    /// Selects the most suitable physical device
    pub fn select(self, instance: &ash::Instance) -> Result<PhysicalDevice> {
        Ok(self.select_all(instance)?.remove(0))
    }
}
