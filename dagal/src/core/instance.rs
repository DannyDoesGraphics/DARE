use std::ops::Deref;

use anyhow::Result;
use ash::vk;
use derivative::Derivative;

use crate::traits::Destructible;

/// Represents a Vulkan Instance
#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct Instance {
    #[derivative(Debug = "ignore")]
    entry: ash::Entry,
    #[derivative(Debug = "ignore")]
    instance: ash::Instance,
}

impl Instance {
    pub fn new(instance_ci: vk::InstanceCreateInfo) -> Result<Self> {
        let entry = unsafe { ash::Entry::load()? };
        let instance = unsafe { entry.create_instance(&instance_ci, None)? };

        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Creating VkInstance {:p}", instance.handle());

        Ok(Self { entry, instance })
    }

    /// Get the [`ash::Entry`]
    pub fn get_entry(&self) -> &ash::Entry {
        &self.entry
    }

    /// Get the [`ash::Instance`]
    pub fn get_instance(&self) -> &ash::Instance {
        &self.instance
    }
}

impl Destructible for Instance {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Destroying VkInstance {:p}", self.instance.handle());

        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}

impl Deref for Instance {
    type Target = ash::Instance;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

#[cfg(feature = "raii")]
impl Drop for Instance {
    fn drop(&mut self) {
        self.destroy();
    }
}
