use bevy_ecs::prelude as becs;
use dagal::allocators::GPUAllocatorImpl;

/// Context that manages core Vulkan device resources
#[derive(Debug, becs::Resource)]
pub struct DeviceContext {
    // Order matters for Drop! Rust drops fields in declaration order
    // We want to drop the most dependent objects first
    pub allocator: GPUAllocatorImpl,
    pub device: dagal::device::LogicalDevice,
    pub physical_device: dagal::device::PhysicalDevice,
    pub debug_messenger: Option<dagal::device::DebugMessenger>,
    pub instance: dagal::core::Instance,
}

impl DeviceContext {
    pub fn new(
        instance: dagal::core::Instance,
        physical_device: dagal::device::PhysicalDevice,
        device: dagal::device::LogicalDevice,
        allocator: GPUAllocatorImpl,
        debug_messenger: Option<dagal::device::DebugMessenger>,
    ) -> Self {
        Self {
            allocator,
            device,
            physical_device,
            debug_messenger,
            instance,
        }
    }
}

impl Drop for DeviceContext {
    fn drop(&mut self) {
        // Wait for the device to be idle before dropping
        unsafe {
            let _ = self.device.get_handle().device_wait_idle();
        }
        tracing::trace!("Dropping DeviceContext");
    }
}
