use crate::traits::Destructible;
use ash::vk;

pub struct ComputePipeline {
    device: crate::device::LogicalDevice,
    handle: vk::Pipeline,
}

impl Destructible for ComputePipeline {
    fn destroy(&mut self) {
        unsafe {
            self.device.get_handle().destroy_pipeline(self.handle, None);
        }
    }
}
