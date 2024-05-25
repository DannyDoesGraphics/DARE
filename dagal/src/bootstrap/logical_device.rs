use anyhow::Result;
use ash::vk;
use derivative::Derivative;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::{c_char, c_void, CString};
use std::ptr;
use std::rc::Rc;

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
    request_queues: Vec<Rc<RefCell<crate::bootstrap::QueueRequest>>>,
    debug_utils: bool,
}

impl<'a> LogicalDeviceBuilder<'a> {
    /// Construct a new logical device builder
    ///
    /// # Example
    /// ```
    /// use ash::vk;
    /// use dagal::traits::*;
    /// let (mut instance, mut stack) = dagal::util::tests::create_vulkan(Default::default());
    /// let queues = vec![
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true)
    /// ];
    /// let physical_device = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_required_queue(queues[0].clone())
    /// .select(&instance).unwrap();
    /// let logical_device = dagal::bootstrap::LogicalDeviceBuilder::new(physical_device.handle)
    /// .add_queue_allocation(queues[0].clone())
    /// .build(&instance)
    /// .unwrap();
    /// stack.push(move || {
    ///     drop(logical_device);
    /// });
    /// stack.flush();
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
    /// let (instance, mut stack) = dagal::util::tests::create_vulkan(Default::default());
    /// let queues = vec![
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true)
    /// ];
    /// let physical_device = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_required_queue(queues[0].clone())
    /// .select(&instance).unwrap();
    /// let logical_device = dagal::bootstrap::LogicalDeviceBuilder::new(physical_device.handle)
    /// .add_queue_allocation(queues[0].clone())
    /// .add_extension(ash::khr::buffer_device_address::NAME.as_ptr())
    /// .build(&instance)
    /// .unwrap();
    /// stack.push(move || {
    ///     drop(logical_device);
    /// });
    /// stack.flush();
    /// ```
    pub fn add_extension(mut self, extension: *const c_char) -> Self {
        self.extensions.insert(crate::util::wrap_c_str(extension));
        self
    }

    /// This really should not be done. Only should be used if you're manually choosing your
    /// physical device
    pub fn add_queue_allocation(
        mut self,
        allocation: Rc<RefCell<crate::bootstrap::QueueRequest>>,
    ) -> Self {
        self.request_queues.push(allocation);
        self
    }

    pub fn build(mut self, instance: &ash::Instance) -> Result<crate::device::LogicalDevice> {
        let mut queue_priorities: Vec<f32> = Vec::new();
        let queue_families = self.physical_device.get_total_queue_families();

        // update queue family counts
        let mut queue_family_counts: HashMap<u32, u32> = HashMap::new();
        let queue_slotting = crate::bootstrap::queue::determine_queue_slotting(
            Vec::from(queue_families),
            self.request_queues.clone(),
        )?;
        let queue_families_used: HashSet<u32> =
            HashSet::from_iter(queue_slotting.iter().flatten().map(|x| x.family_index));
        for queue_slot in queue_slotting.iter().flatten() {
            queue_family_counts
                .entry(queue_slot.family_index)
                .and_modify(|q| *q += queue_slot.count)
                .or_insert(queue_slot.count);
        }

        let queue_cis: Vec<vk::DeviceQueueCreateInfo> = queue_family_counts
            .iter()
            .filter_map(|(queue_family_index, queue_count)| {
                if *queue_count as usize > queue_priorities.len() {
                    queue_priorities.resize(*queue_count as usize, 1.0)
                }
                if *queue_count == 0 {
                    None
                } else {
                    Some(vk::DeviceQueueCreateInfo {
                        s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
                        p_next: ptr::null(),
                        flags: vk::DeviceQueueCreateFlags::empty(),
                        queue_family_index: *queue_family_index,
                        queue_count: *queue_count,
                        p_queue_priorities: queue_priorities.as_ptr(),
                        _marker: Default::default(),
                    })
                }
            })
            .collect();
        let c_strings: Vec<CString> = self
            .extensions
            .iter()
            .map(|ext| {
                println!("loading: {:?}", ext);
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
        let features_2 = vk::PhysicalDeviceFeatures2 {
            s_type: vk::StructureType::PHYSICAL_DEVICE_FEATURES_2,
            p_next: &mut self.features_1_1 as *mut _ as *mut c_void,
            //p_next: ptr::null_mut(),
            features: self.features_1_0,
            _marker: Default::default(),
        };

        #[allow(deprecated)]
        let device_ci = vk::DeviceCreateInfo {
            s_type: vk::StructureType::DEVICE_CREATE_INFO,
            p_next: &features_2 as *const _ as *const c_void,
            flags: vk::DeviceCreateFlags::empty(),
            queue_create_info_count: queue_cis.len() as u32,
            p_queue_create_infos: queue_cis.as_ptr(),
            enabled_layer_count: 0,
            pp_enabled_layer_names: ptr::null(),
            enabled_extension_count: c_ptrs.len() as u32,
            pp_enabled_extension_names: c_ptrs.as_ptr(),
            p_enabled_features: ptr::null(),
            _marker: Default::default(),
        };
        let device = crate::device::LogicalDevice::new(
            crate::device::LogicalDeviceCreateInfo {
                instance,
                physical_device: self.physical_device,
                device_ci,
                queue_families: queue_families_used.into_iter().collect::<Vec<u32>>(),
                enabled_extensions: self.extensions.iter().map(|data| data.to_string_lossy().to_string()).collect::<HashSet<String>>(),
                debug_utils: self.debug_utils,
            },
        )?;
        // reallocate back the queues
        for (queue_request, queue_allocations) in
            self.request_queues.into_iter().zip(queue_slotting.iter())
        {
            for allocation in queue_allocations.iter() {
                queue_request
                    .borrow_mut()
                    .queues
                    .push(device.get_queue(&vk::DeviceQueueInfo2 {
                        s_type: vk::StructureType::DEVICE_QUEUE_INFO_2,
                        p_next: ptr::null(),
                        flags: vk::DeviceQueueCreateFlags::empty(),
                        queue_family_index: allocation.family_index,
                        queue_index: allocation.index,
                        _marker: Default::default(),
                    }));
            }
        }
        Ok(device)
    }
}

impl<'a> From<crate::bootstrap::PhysicalDevice> for LogicalDeviceBuilder<'a> {
    /// Construct a new logical device builder from a [`bootstrap::Bootstrap`](crate::bootstrap::PhysicalDevice)
    ///
    /// # Examples
    /// ```
    /// use ash::vk;
    /// let (instance, mut stack) = dagal::util::tests::create_vulkan(Default::default());
    /// let queues = vec![
    ///     dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true)
    /// ];
    /// let physical_device = dagal::bootstrap::PhysicalDeviceSelector::default()
    /// .add_required_queue(queues[0].clone())
    /// .select(&instance).unwrap();
    /// let logical_device = dagal::bootstrap::LogicalDeviceBuilder::from(physical_device)
    /// .build(&instance)
    /// .unwrap();
    /// let queue = queues.get(0).unwrap().borrow();
    /// assert_eq!(queue.get_queues().len(), 1);
    /// assert_eq!(queue.get_queues()[0].get_index(), 0);
    /// stack.flush();
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
