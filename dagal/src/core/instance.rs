use std::ops::Deref;

use anyhow::Result;
use ash::vk;
use derivative::Derivative;

use crate::traits::Destructible;

/// Represents a Vulkan Instance
#[derive(Derivative)]
#[derivative(Debug)]
pub struct Instance {
    #[derivative(Debug = "ignore")]
    entry: ash::Entry,
    #[derivative(Debug = "ignore")]
    instance: ash::Instance,
    #[derivative(Debug = "ignore")]
    debug_messenger: Option<crate::device::DebugMessenger>,
}

impl Instance {
    pub fn new(instance_ci: vk::InstanceCreateInfo) -> Result<Self> {
        let entry = unsafe { ash::Entry::load()? };
        let instance = unsafe { entry.create_instance(&instance_ci, None)? };

        #[cfg(feature = "log-lifetimes")]
        log::trace!("Creating VkInstance {:p}", instance.handle());

        Ok(Self {
            entry,
            instance,
            debug_messenger: None,
        })
    }

    pub fn attach_debug_messenger(&mut self) -> Result<()> {
        self.debug_messenger = None;
        self.debug_messenger = Some(crate::device::DebugMessenger::new(
            &self.entry,
            &self.instance,
        )?);
        Ok(())
    }

    pub fn has_debug_messenger(&self) -> bool {
        self.debug_messenger.is_some()
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
        log::trace!("Destroying VkInstance {:p}", self.instance.handle());
        self.debug_messenger = None;
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

impl Drop for Instance {
    fn drop(&mut self) {
        self.destroy();
    }
}
