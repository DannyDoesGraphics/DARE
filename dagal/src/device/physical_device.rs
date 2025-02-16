use crate::bootstrap::app_info::{Expected, QueueRequest};
use crate::device::QueueInfo;
use crate::traits::AsRaw;
use ash;
use ash::vk;
use std::cmp::Ordering;
use std::ops::Deref;

#[derive(Clone, Debug)]
pub struct PhysicalDevice {
    /// Handle to underlying physical device
    handle: vk::PhysicalDevice,

    /// Properties of the [`vk::PhysicalDevice`]
    properties: vk::PhysicalDeviceProperties,

    /// All enabled extensions
    enabled_extensions: Vec<String>,

    /// All active queues in use
    active_queues: Vec<QueueInfo>,

    /// Queue families of the [`vk::PhysicalDevice`]
    available_queue_families: Vec<vk::QueueFamilyProperties>,
}

fn allocated_preferred_queues(
    families_cap: &mut [u32],
    family_infos: &[vk::QueueFamilyProperties2],
    request: &QueueRequest,
    needed: u32,
) -> u32 {
    let mut remaining = needed;

    for (i, family) in family_infos.iter().enumerate() {
        if remaining == 0 {
            break;
        }
        if !request.contains_required(family) {
            continue;
        }

        let available = families_cap[i];
        let to_take = available.min(remaining);
        if to_take > 0 {
            families_cap[i] -= to_take;
            remaining -= to_take;
        }
    }

    let allocated = needed - remaining;
    allocated
}

impl PhysicalDevice {
    /// References the underlying [`VkPhysicalDevice`](vk::PhysicalDevice)
    pub fn get_handle(&self) -> &vk::PhysicalDevice {
        &self.handle
    }

    /// Copies the underlying [`VkPhysicalDevice`](vk::PhysicalDevice)
    pub fn handle(&self) -> vk::PhysicalDevice {
        self.handle
    }

    /// Get the properties of the physical device
    pub fn get_properties(&self) -> &vk::PhysicalDeviceProperties {
        &self.properties
    }

    /// Get all actively used queues
    pub fn get_active_queues(&self) -> &[QueueInfo] {
        self.active_queues.as_slice()
    }

    /// Get all actively used extensions
    pub fn get_extensions(&self) -> &[String] {
        self.enabled_extensions.as_slice()
    }

    /// Get the queue families
    pub fn get_total_queue_families(&self) -> &[vk::QueueFamilyProperties] {
        self.available_queue_families.as_slice()
    }

    /// Creates a new physical device
    pub fn new(
        instance: &ash::Instance,
        handle: vk::PhysicalDevice,
        enabled_extensions: Vec<String>,
        active_queues: Vec<QueueInfo>,
    ) -> Self {
        let mut properties_2 = vk::PhysicalDeviceProperties2::default();
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(handle) };
        unsafe {
            instance.get_physical_device_properties2(handle, &mut properties_2);
        }
        Self {
            handle,
            properties: properties_2.properties,
            enabled_extensions,
            active_queues,
            available_queue_families: queue_families,
        }
    }

    /// Selects the most suitable device
    pub fn select<Window: crate::wsi::DagalWindow>(
        instance: &crate::core::Instance,
        surface: Option<&crate::wsi::Surface>,
        mut settings: crate::bootstrap::app_info::AppSettings<Window>,
    ) -> anyhow::Result<Self> {
        use std::ffi::c_void;

        struct PhysicalDeviceInfo<'a> {
            physical_device: vk::PhysicalDevice,
            properties_1_3: vk::PhysicalDeviceVulkan13Properties<'a>,
            properties_1_2: vk::PhysicalDeviceVulkan12Properties<'a>,
            properties_1_1: vk::PhysicalDeviceVulkan11Properties<'a>,
            properties: vk::PhysicalDeviceProperties2<'a>,
            queue_family_properties: Vec<vk::QueueFamilyProperties2<'a>>,
            /// Used to determine the heuristic of a given physical device
            heuristic: u32,
            /// Extensions to be actually used
            extensions: Vec<String>,
            /// Queues actually used
            queues: Vec<QueueInfo>,
        }

        let suitable_device: Option<PhysicalDeviceInfo> = unsafe {
            /// Find all physical devices
            let physical_devices: Vec<PhysicalDeviceInfo> = instance
                .enumerate_physical_devices()?
                .into_iter()
                .map(|physical_device| {
                    // Get device properties
                    let mut properties_1_3: vk::PhysicalDeviceVulkan13Properties =
                        Default::default();
                    properties_1_3.s_type =
                        vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_3_PROPERTIES;
                    let mut properties_1_2: vk::PhysicalDeviceVulkan12Properties =
                        Default::default();
                    properties_1_2.s_type =
                        vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_2_PROPERTIES;
                    properties_1_2.p_next = &mut properties_1_3 as *mut _ as *mut c_void;
                    let mut properties_1_1: vk::PhysicalDeviceVulkan11Properties =
                        Default::default();
                    properties_1_1.s_type =
                        vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_PROPERTIES;
                    properties_1_1.p_next = &mut properties_1_2 as *mut _ as *mut c_void;
                    let mut properties: vk::PhysicalDeviceProperties2 =
                        vk::PhysicalDeviceProperties2 {
                            s_type: vk::StructureType::PHYSICAL_DEVICE_PROPERTIES_2,
                            p_next: &mut properties_1_1 as *mut _ as *mut c_void,
                            properties: Default::default(),
                            _marker: Default::default(),
                        };
                    instance.get_physical_device_properties2(physical_device, &mut properties);
                    let mut queue_family_properties: Vec<vk::QueueFamilyProperties2> =
                        vec![
                            Default::default();
                            instance
                                .get_physical_device_queue_family_properties2_len(physical_device)
                        ];
                    instance.get_physical_device_queue_family_properties2(
                        physical_device,
                        &mut queue_family_properties,
                    );
                    PhysicalDeviceInfo {
                        physical_device,
                        properties_1_3,
                        properties_1_2,
                        properties_1_1,
                        properties,
                        queue_family_properties,
                        heuristic: 0,
                        extensions: Vec::new(),
                        queues: Vec::new(),
                    }
                })
                .collect::<Vec<PhysicalDeviceInfo>>();

            physical_devices
                .into_iter()
                .filter_map(|mut pd| {
                    let props = instance.get_physical_device_properties(pd.physical_device);
                    let dev_type = props.device_type;

                    // Check dedicated requirement
                    match settings.gpu_requirements.dedicated {
                        Expected::Required(required_is_discrete) => {
                            let this_is_discrete = dev_type == vk::PhysicalDeviceType::DISCRETE_GPU;
                            if this_is_discrete != required_is_discrete {
                                // mismatch => not suitable
                                return None;
                            }
                        }
                        Expected::Preferred(pref_is_discrete) => {
                            let this_is_discrete = dev_type == vk::PhysicalDeviceType::DISCRETE_GPU;
                            if this_is_discrete == pref_is_discrete {
                                // arbitrary heuristic bonus
                                pd.heuristic += 50;
                            }
                        }
                    }

                    // Attempt to allocate all queues
                    let mut family_capacity: Vec<u32> = pd
                        .queue_family_properties
                        .iter()
                        .map(|f| f.queue_family_properties.queue_count)
                        .collect();

                    let mut family_offsets: Vec<u32> = vec![0; family_capacity.len()];
                    for queue_req in &settings.gpu_requirements.queues {
                        if let Expected::Required(needed) = queue_req.count {
                            // Try to find **one** family that can satisfy all needed queues
                            // for this request in a single family. If found, allocate them.
                            let mut allocated = false;
                            for (fam_idx, fam_props) in
                                pd.queue_family_properties.iter().enumerate()
                            {
                                // Must match required flags
                                if !queue_req.contains_required(fam_props) {
                                    continue;
                                }
                                if family_capacity[fam_idx] >= needed {
                                    // Allocate [0..needed) from this family's next offset
                                    let start_offset = family_offsets[fam_idx];
                                    for i in 0..needed {
                                        let queue_index = start_offset + i;
                                        pd.queues.push(QueueInfo {
                                            family_index: fam_idx as u32,
                                            index: queue_index,
                                            strict: queue_req.strict,
                                            queue_flags: fam_props
                                                .queue_family_properties
                                                .queue_flags,
                                            can_present: false,
                                        });
                                    }
                                    family_offsets[fam_idx] += needed;
                                    family_capacity[fam_idx] -= needed;
                                    allocated = true;
                                    break;
                                }
                            }
                            if !allocated {
                                // Could not fulfill a required request
                                return None;
                            }
                        }
                    }

                    // --- Allocate PREFERRED next (partially if needed) ---
                    for queue_req in &settings.gpu_requirements.queues {
                        if let Expected::Preferred(needed) = queue_req.count {
                            let mut remaining = needed;
                            // We allow partial allocation across multiple families
                            for (fam_idx, fam_props) in
                                pd.queue_family_properties.iter().enumerate()
                            {
                                if remaining == 0 {
                                    break;
                                }
                                // Must at least match the "required" portion of the request
                                if !queue_req.contains_required(fam_props) {
                                    continue;
                                }

                                let available = family_capacity[fam_idx];
                                if available == 0 {
                                    continue;
                                }
                                let to_take = available.min(remaining);
                                // Allocate these `to_take` queues
                                let start_offset = family_offsets[fam_idx];
                                for i in 0..to_take {
                                    let queue_index = start_offset + i;
                                    pd.queues.push(QueueInfo {
                                        family_index: fam_idx as u32,
                                        index: queue_index,
                                        strict: queue_req.strict,
                                        queue_flags: fam_props.queue_family_properties.queue_flags,
                                        can_present: false,
                                    });
                                }
                                family_offsets[fam_idx] += to_take;
                                family_capacity[fam_idx] -= to_take;
                                remaining -= to_take;
                            }

                            // Optional: Add some heuristic points for each queue allocated
                            // For example, reward partial allocations:
                            let allocated_preferred = needed - remaining;
                            // e.g. +10 points for each queue allocated
                            pd.heuristic += allocated_preferred * 10;
                        }
                    }
                    Some(pd)
                })
                .max_by_key(|pd| pd.heuristic)
        };

        suitable_device
            .map(|pd| {
                PhysicalDevice::new(
                    instance.get_instance(),
                    pd.physical_device,
                    pd.extensions,
                    pd.queues,
                )
            })
            .ok_or_else(|| crate::error::DagalError::NoPhysicalDevice.into())
    }
}

impl AsRaw for PhysicalDevice {
    type RawType = vk::PhysicalDevice;

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

impl Deref for PhysicalDevice {
    type Target = vk::PhysicalDevice;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}
