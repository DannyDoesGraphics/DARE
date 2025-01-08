/// Contains a dagal context mainly used by the render graph
#[derive(Debug)]
pub struct Context {
    pub instance: crate::core::Instance,
    pub physical_device: crate::device::PhysicalDevice,
    pub device: crate::device::LogicalDevice,
}