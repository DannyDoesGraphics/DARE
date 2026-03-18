use dagal::ash::vk;
use dagal::bootstrap::app_info::Expected;
use dagal::bootstrap::app_info::QueueRequest;
use dagal::bootstrap::init::ContextInit;

/// Headless Vulkan instance
#[derive(Debug)]
pub struct TestContext {
    allocator: dagal::allocators::GPUAllocatorImpl,
    device: dagal::device::LogicalDevice,
    physical_device: dagal::device::PhysicalDevice,
    instance: dagal::core::Instance,
}

impl TestContext {
    pub fn new() -> anyhow::Result<Self> {
        let (instance, physical_device, _surface, device, allocator) =
            dagal::bootstrap::init::Context::init(dagal::bootstrap::app_info::AppSettings {
                name: "Test".to_string(),
                version: 0,
                engine_name: "Test".to_string(),
                engine_version: 0,
                api_version: (1, 4, 0, 0),
                enable_validation: true,
                debug_utils: cfg!(debug_assertions),
                raw_display_handle: None,
                raw_window_handle: None,
                surface_format: None,
                present_mode: None,
                gpu_requirements: dagal::bootstrap::app_info::GPURequirements {
                    dedicated: Expected::Preferred(true),
                    features: dagal::ash::vk::PhysicalDeviceFeatures::default(),
                    features_1: dagal::ash::vk::PhysicalDeviceVulkan11Features::default(),
                    features_2: dagal::ash::vk::PhysicalDeviceVulkan12Features::default(),
                    features_3: dagal::ash::vk::PhysicalDeviceVulkan13Features::default(),
                    device_extensions: Vec::new(),
                    queues: vec![QueueRequest {
                        strict: false,
                        queue_type: vec![Expected::Required(
                            vk::QueueFlags::GRAPHICS
                                | vk::QueueFlags::TRANSFER
                                | vk::QueueFlags::COMPUTE,
                        )]
                        .into(),
                        count: Expected::Required(1),
                    }],
                },
            })
            .unwrap();

        Ok(Self {
            instance,
            physical_device,
            device,
            allocator,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Literally test if the context can be created
    #[tokio::test]
    async fn test_new() {
        let context = TestContext::new().unwrap();
        drop(context);
    }
}
