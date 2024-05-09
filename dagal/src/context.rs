use derivative::Derivative;

/// This represents a rendering context

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Context {
    #[derivative(Debug = "ignore")]
    device: crate::device::LogicalDevice,
}

impl Context {
    pub fn get_device(&self) -> crate::device::LogicalDevice {
        self.device.clone()
    }
}

pub struct ContextBuilder {}
