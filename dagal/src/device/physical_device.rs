use crate::traits::AsRaw;
use ash;
use ash::vk;
use std::ops::Deref;

#[derive(Clone, Debug)]
pub struct PhysicalDevice {
    /// Handle to underlying physical device
    handle: vk::PhysicalDevice,

    /// Properties of the [`vk::PhysicalDevice`]
    properties: vk::PhysicalDeviceProperties,

    /// Queue families of the [`vk::PhysicalDevice`]
    available_queue_families: Vec<vk::QueueFamilyProperties>,
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

    /// Get the queue families
    pub fn get_total_queue_families(&self) -> &[vk::QueueFamilyProperties] {
        self.available_queue_families.as_slice()
    }

    /// Creates a new physical device
    pub fn new(instance: &ash::Instance, handle: vk::PhysicalDevice) -> Self {
        let mut properties_2 = vk::PhysicalDeviceProperties2::default();
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(handle) };
        unsafe {
            instance.get_physical_device_properties2(handle, &mut properties_2);
        }
        Self {
            handle,
            properties: properties_2.properties,
            available_queue_families: queue_families,
        }
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
