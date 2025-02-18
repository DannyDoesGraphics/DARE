use crate::device::queue::QueueInfo;
use ash::vk;
use std::ffi::CString;
use std::fmt::Debug;

/// Indicates what the expectation of such an input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected<T: Clone + PartialEq + Eq + Debug> {
    Required(T),
    /// Ordered from preferred first to least preferred
    Preferred(T),
}

impl<T: Clone + PartialEq + Eq + Debug> Expected<T> {
    pub fn is_preferred(&self) -> bool {
        match self {
            Expected::Preferred(_) => true,
            _ => false,
        }
    }

    pub fn is_required(&self) -> bool {
        match self {
            Expected::Required(_) => true,
            _ => false,
        }
    }

    pub fn clone_as(&self) -> T {
        match self {
            Expected::Required(a) => a.clone(),
            Expected::Preferred(a) => a.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueRequest {
    /// If the required queue flags should be strictly matched to the queue type
    pub strict: bool,
    /// Capabilities requested from said queue in question
    pub queue_type: Box<[Expected<vk::QueueFlags>]>,
    /// Number of queues requested
    ///
    /// By default, [`Expected::DontCare`], uses the maximum # of queues in the family
    pub count: Expected<u32>,
}

impl QueueRequest {
    /// Check if any given [`vk::QueueFamilyProperties2`] matches
    pub fn contains_required(&self, family_properties: &vk::QueueFamilyProperties2) -> bool {
        let mut required_flags: vk::QueueFlags = vk::QueueFlags::empty();
        for expected_flags in self.queue_type.iter() {
            match expected_flags {
                Expected::Required(expected_flags) => {
                    required_flags |= *expected_flags;
                }
                Expected::Preferred(_) => {}
            }
        }

        if self.strict && required_flags != family_properties.queue_family_properties.queue_flags {
            false
        } else if !self.strict
            && family_properties.queue_family_properties.queue_flags & required_flags
                != required_flags
        {
            false
        } else {
            true
        }
    }

    /// Get the # of preferred flags which are found in a [`vk::QueueFamilyProperties2`]
    pub fn get_matching_preferred(&self, family_properties: &vk::QueueFamilyProperties2) -> u32 {
        self.queue_type
            .iter()
            .filter(|expected_flags| match expected_flags {
                Expected::Preferred(expected_flags) => family_properties
                    .queue_family_properties
                    .queue_flags
                    .contains(*expected_flags),
                _ => false,
            })
            .count() as u32
    }
}

#[derive(Debug)]
pub struct GPURequirements {
    /// Whether a dedicated GPU is required
    pub dedicated: Expected<bool>,
    /// Features expected of a GPU
    pub features: vk::PhysicalDeviceFeatures,
    pub features_1: vk::PhysicalDeviceVulkan11Features<'static>,
    pub features_2: vk::PhysicalDeviceVulkan12Features<'static>,
    pub features_3: vk::PhysicalDeviceVulkan13Features<'static>,
    /// Device extensions that should be enabled on the GPU
    pub device_extensions: Vec<Expected<String>>,
    /// Queues expected of a device
    pub queues: Vec<QueueRequest>,
}

#[derive(Debug)]
pub struct AppSettings<'a, Window: crate::wsi::DagalWindow> {
    /// Name of application
    pub name: String,
    /// Application version
    pub version: u32,
    /// Name of engine
    pub engine_name: String,
    /// Version of engine
    pub engine_version: u32,
    /// Api version
    pub api_version: (u32, u32, u32, u32),
    /// Enable validation layers
    pub enable_validation: bool,
    /// Enable debug utils
    pub debug_utils: bool,
    /// Optional window reference
    pub window: Option<&'a Window>,
    /// Surface formats expected
    pub surface_format: Option<Expected<vk::SurfaceFormatKHR>>,
    /// Preferred present mode, ordered from most preferred to least
    pub present_mode: Option<Expected<vk::PresentModeKHR>>,
    /// Minimum requirements the GPU should be expected to have
    pub gpu_requirements: GPURequirements,
}
