use std::sync::Arc;
/// Contains a dagal context mainly used by the render graph
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Context {
    pub instance: Arc<crate::core::Instance>,
    pub physical_device: crate::device::PhysicalDevice,
    pub device: Arc<crate::device::LogicalDevice>,
}
