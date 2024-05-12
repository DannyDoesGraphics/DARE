use ash::vk;

#[derive(Debug, Clone)]
pub struct DynamicRenderContext {
	handle: vk::CommandBuffer,
	device: crate::device::LogicalDevice,
}
