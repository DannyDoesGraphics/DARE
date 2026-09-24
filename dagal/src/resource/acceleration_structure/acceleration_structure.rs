use ash::vk;

#[derive(Debug)]
pub struct AccelerationStructure {
    handle: vk::AccelerationStructureKHR,
    device: crate::device::LogicalDevice,
}

impl AccelerationStructure {
    pub fn new(_ci: vk::AccelerationStructureCreateInfoKHR) -> crate::Result<Self> {
        todo!()
    }
}
