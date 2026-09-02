/// Realistically, you should always prefer to use the vulkan configurator over this module, however
/// this module exists for primarily unit testing only. This modules
use anyhow::Result;
use ash::vk;
use derivative::Derivative;

use crate::traits::Destructible;

/// Represents a [`VkDebugUtilsMessengerEXT`](ash::ext::debug_utils)
#[derive(Derivative)]
#[derivative(Debug)]
pub struct DebugMessenger {
    #[derivative(Debug = "ignore")]
    handle: vk::DebugUtilsMessengerEXT,
    #[derivative(Debug = "ignore")]
    ext: ash::ext::debug_utils::Instance,
}

impl DebugMessenger {
    pub fn new(entry: &ash::Entry, instance: &ash::Instance) -> Result<Self> {
        let ext = ash::ext::debug_utils::Instance::new(entry, instance);
        let debug_ci = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
            )
            .pfn_user_callback(Some(vk_debug_callback));
        let handle = unsafe { ext.create_debug_utils_messenger(&debug_ci, None)? };

        #[cfg(feature = "log-lifetimes")]
        log::trace!("Creating VkDebugUtilsMessenger {:p}", handle);

        Ok(Self { handle, ext })
    }
}

impl Destructible for DebugMessenger {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        log::trace!("Destroying VkDebugUtilsMessenger {:p}", self.handle);

        unsafe { self.ext.destroy_debug_utils_messenger(self.handle, None) }
    }
}

impl Drop for DebugMessenger {
    fn drop(&mut self) {
        self.destroy();
    }
}

/// the callback function used in Debug Utils.
/// thanks phobos https://github.com/NotAPenguin0/phobos-rs/blob/2a1e539611bb3ede5c2d7978300353630c7c553b/src/core/debug.rs#L75-L129
extern "system" fn vk_debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    msg_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let callback_data = unsafe { *p_callback_data };
    let message_id_number = callback_data.message_id_number;
    let message_id_name = crate::util::wrap_c_str(callback_data.p_message_id_name);
    let message_id_name = message_id_name.to_string_lossy();
    let message = crate::util::wrap_c_str(callback_data.p_message);
    let message = message.to_string_lossy();

    let formatted = format!("[{msg_type:?}]: {message_id_name} ({message_id_number}): {message}");
    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        log::error!("{formatted}");
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        log::warn!("{formatted}");
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::INFO) {
        log::info!("{formatted}");
    } else {
        log::trace!("{formatted}");
    }

    vk::FALSE
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use ash::vk;

    use crate::resource::traits::Resource;
    use crate::util::tests::TestHarness;

    static LOGGED_ERRORS: AtomicUsize = AtomicUsize::new(0);

    struct CountingLogger;

    impl log::Log for CountingLogger {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Error
        }

        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                LOGGED_ERRORS.fetch_add(1, Ordering::Relaxed);
            }
        }

        fn flush(&self) {}
    }

    #[test]
    fn instance_builder_attaches_a_messenger_when_validating() {
        let _ = log::set_logger(&CountingLogger);
        log::set_max_level(log::LevelFilter::Error);

        let instance = crate::bootstrap::InstanceBuilder::new()
            .set_validation(true)
            .set_vulkan_version((1, 3, 0))
            .build()
            .unwrap();
        assert!(
            instance.has_debug_messenger(),
            "a validating InstanceBuilder left the instance without a debug messenger"
        );
    }

    #[test]
    fn attached_messenger_reports_validation_errors() {
        let _ = log::set_logger(&CountingLogger);
        log::set_max_level(log::LevelFilter::Error);

        let harness = TestHarness::headless().build().unwrap();
        let before = LOGGED_ERRORS.load(Ordering::Relaxed);

        let _ = crate::resource::Buffer::new(crate::resource::BufferCreateInfo::NewEmptyBuffer::<
            crate::allocators::GPUAllocatorImpl,
        > {
            device: harness.device(),
            name: None,
            allocator: &harness.allocator(),
            size: 0,
            memory_type: crate::allocators::MemoryLocation::GpuOnly,
            usage_flags: vk::BufferUsageFlags::TRANSFER_DST,
        });

        assert!(
            LOGGED_ERRORS.load(Ordering::Relaxed) > before,
            "the debug messenger did not report a known invalid buffer creation"
        );
    }
}
