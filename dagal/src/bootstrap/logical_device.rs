use std::collections::{HashMap, HashSet};
use std::ffi::{CString, c_char, c_void};
use std::ptr;

use anyhow::Result;
use ash::vk;
use derivative::Derivative;

/// Builds a logical device
#[derive(Derivative)]
#[derivative(Debug)]
pub struct LogicalDeviceBuilder<'a> {
    physical_device: crate::device::PhysicalDevice,
    features_1_0: vk::PhysicalDeviceFeatures,
    features_1_1: vk::PhysicalDeviceVulkan11Features<'a>,
    features_1_2: vk::PhysicalDeviceVulkan12Features<'a>,
    features_1_3: vk::PhysicalDeviceVulkan13Features<'a>,
    extensions: HashSet<CString>,
    request_queues: Vec<crate::bootstrap::QueueRequest>,
    debug_utils: bool,
}

impl<'a> LogicalDeviceBuilder<'a> {
    /// Construct a new logical device builder
    ///
    /// # Example
    /// ```
    /// use ash::vk;
    /// use dagal::traits::*;
    /// let ctx = dagal::util::tests::TestHarness::headless().build().unwrap();
    /// let queues = vec![
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true)
    /// ];
    /// let physical_device = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_required_queue(queues[0].clone())
    /// .select(ctx.instance()).unwrap();
    /// let logical_device = dagal::bootstrap::LogicalDeviceBuilder::new(physical_device.handle)
    /// .add_queue_allocation(queues[0].clone())
    /// .build(ctx.instance())
    /// .unwrap();
    /// drop(logical_device);
    /// ```
    pub fn new(physical_device: crate::device::PhysicalDevice) -> Self {
        Self {
            physical_device,
            features_1_0: Default::default(),
            features_1_1: Default::default(),
            features_1_2: Default::default(),
            features_1_3: Default::default(),
            extensions: HashSet::new(),
            request_queues: vec![],
            debug_utils: false,
        }
    }

    pub fn debug_utils(mut self, enabled: bool) -> Self {
        self.debug_utils = enabled;
        self
    }

    pub fn attach_feature_1_0(mut self, feature: vk::PhysicalDeviceFeatures) -> Self {
        self.features_1_0 = feature;
        self
    }

    pub fn attach_feature_1_1(mut self, feature: vk::PhysicalDeviceVulkan11Features<'a>) -> Self {
        self.features_1_1 = feature;
        self
    }

    pub fn attach_feature_1_2(mut self, feature: vk::PhysicalDeviceVulkan12Features<'a>) -> Self {
        self.features_1_2 = feature;
        self
    }

    pub fn attach_feature_1_3(mut self, feature: vk::PhysicalDeviceVulkan13Features<'a>) -> Self {
        self.features_1_3 = feature;
        self
    }

    /// Adds an extension to enable
    ///
    /// # Examples
    /// Add buffer device address extension
    /// ```
    /// use ash::vk;
    /// let ctx = dagal::util::tests::TestHarness::headless().build().unwrap();
    /// let queues = vec![
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true)
    /// ];
    /// let physical_device = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_required_queue(queues[0].clone())
    /// .select(ctx.instance()).unwrap();
    /// let logical_device = dagal::bootstrap::LogicalDeviceBuilder::new(physical_device.handle)
    /// .add_queue_allocation(queues[0].clone())
    /// .add_extension(ash::khr::buffer_device_address::NAME.as_ptr())
    /// .build(ctx.instance())
    /// .unwrap();
    /// drop(logical_device);
    /// ```
    pub fn add_extension(mut self, extension: *const c_char) -> Self {
        self.extensions.insert(crate::util::wrap_c_str(extension));
        self
    }

    /// This really should not be done. Only should be used if you're manually choosing your
    /// physical device
    pub fn add_queue_allocation(mut self, allocation: crate::bootstrap::QueueRequest) -> Self {
        self.request_queues.push(allocation);
        self
    }

    pub fn build(
        mut self,
        instance: &ash::Instance,
    ) -> Result<(crate::device::LogicalDevice, Vec<crate::device::Queue>)> {
        let mut queue_priorities: Vec<f32> = Vec::new();
        let queue_families = self.physical_device.get_total_queue_families();

        // update queue family counts
        let mut queue_family_counts: HashMap<u32, u32> = HashMap::new();
        let queue_slotting = crate::bootstrap::queue::determine_queue_slotting(
            Vec::from(queue_families),
            self.request_queues.clone(),
        )?;
        let _queue_families_used: HashSet<u32> =
            HashSet::from_iter(queue_slotting.iter().flatten().map(|x| x.family_index));
        for queue_slot in queue_slotting.iter().flatten() {
            queue_family_counts
                .entry(queue_slot.family_index)
                .and_modify(|q| *q += queue_slot.count)
                .or_insert(queue_slot.count);
        }

        let max_queue_count = queue_family_counts.values().copied().max().unwrap_or(0) as usize;
        queue_priorities.resize(max_queue_count, 1.0);
        let mut queue_cis: Vec<vk::DeviceQueueCreateInfo> = Vec::new();
        for (queue_family_index, queue_count) in queue_family_counts.iter() {
            if *queue_count == 0 {
                continue;
            }
            queue_cis.push(
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(*queue_family_index)
                    .queue_priorities(&queue_priorities[..*queue_count as usize]),
            );
        }
        let c_strings: Vec<CString> = self
            .extensions
            .iter()
            .map(|ext| {
                println!("loading: {ext:?}");
                CString::new(ext.clone()).unwrap()
            })
            .collect();
        let c_ptrs: Vec<*const c_char> = c_strings.iter().map(|ext| ext.as_ptr()).collect();
        // Assemble features
        self.features_1_3.s_type = vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_3_FEATURES;
        self.features_1_2.s_type = vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_2_FEATURES;
        self.features_1_1.s_type = vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_FEATURES;

        self.features_1_3.p_next = ptr::null_mut();
        self.features_1_2.p_next = &mut self.features_1_3 as *mut _ as *mut c_void;
        self.features_1_1.p_next = &mut self.features_1_2 as *mut _ as *mut c_void;
        let mut features_2 = vk::PhysicalDeviceFeatures2::default();
        features_2.p_next = &mut self.features_1_1 as *mut _ as *mut c_void;
        features_2.features = self.features_1_0;

        #[allow(deprecated)]
        let device_ci = {
            let mut device_ci = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_cis)
                .enabled_extension_names(&c_ptrs);
            device_ci.p_next = &features_2 as *const _ as *const c_void;
            device_ci
        };

        let device = crate::device::LogicalDevice::new(crate::device::LogicalDeviceCreateInfo {
            instance,
            physical_device: self.physical_device,
            device_ci,
            debug_utils: self.debug_utils,
        })?;
        let mut queues = Vec::new();
        // reallocate back the queues
        for (queue_request, queue_allocations) in
            self.request_queues.into_iter().zip(queue_slotting.iter())
        {
            for allocation in queue_allocations.iter() {
                let queue_flags: vk::QueueFlags = queue_request.family_flags;
                let dedicated: bool = queue_request.dedicated;
                queues.push(unsafe {
                    device.get_queue(
                        &vk::DeviceQueueInfo2::default()
                            .queue_family_index(allocation.family_index)
                            .queue_index(allocation.index),
                        queue_flags,
                        dedicated,
                        true,
                    )
                });
            }
        }
        Ok((device, queues))
    }
}

impl From<crate::bootstrap::PhysicalDevice> for LogicalDeviceBuilder<'_> {
    /// Construct a new logical device builder from a [`bootstrap::Bootstrap`](crate::bootstrap::PhysicalDevice)
    ///
    /// # Examples
    /// ```
    /// use ash::vk;
    /// let ctx = dagal::util::tests::TestHarness::headless().build().unwrap();
    /// let queues = vec![
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true)
    /// ];
    /// let physical_device = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_required_queue(queues[0].clone())
    /// .select(ctx.instance()).unwrap();
    /// let logical_device = dagal::bootstrap::LogicalDeviceBuilder::from(physical_device)
    /// .build(ctx.instance())
    /// .unwrap();
    /// // Device created successfully with requested queues
    /// drop(logical_device);
    /// ```
    fn from(value: crate::bootstrap::PhysicalDevice) -> Self {
        Self {
            physical_device: value.handle,
            features_1_0: Default::default(),
            features_1_1: Default::default(),
            features_1_2: Default::default(),
            features_1_3: Default::default(),
            extensions: value.extensions_enabled,
            request_queues: value.queue_requests,
            debug_utils: false,
        }
    }
}
