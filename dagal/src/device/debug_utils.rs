/// Realistically, you should always prefer to use the vulkan configurator over this module, however
/// this module exists for primarily unit testing only. This modules
use std::ptr;

use anyhow::Result;
use ash::vk;
use derivative::Derivative;
use tracing::trace;

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
		let debug_ci = vk::DebugUtilsMessengerCreateInfoEXT {
			s_type: vk::StructureType::DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
			p_next: ptr::null(),
			flags: vk::DebugUtilsMessengerCreateFlagsEXT::empty(),
			message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
				| vk::DebugUtilsMessageSeverityFlagsEXT::INFO
				| vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
				| vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
			message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
				| vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
			pfn_user_callback: Some(vk_debug_callback),
			p_user_data: ptr::null_mut(),
			_marker: Default::default(),
		};
		let handle = unsafe { ext.create_debug_utils_messenger(&debug_ci, None)? };

		#[cfg(feature = "log-lifetimes")]
		trace!("Creating VkDebugUtilsMessenger {:p}", handle);

		Ok(Self { handle, ext })
	}
}

impl Destructible for DebugMessenger {
	fn destroy(&mut self) {
		#[cfg(feature = "log-lifetimes")]
		trace!("Destroying VkDebugUtilsMessenger {:p}", self.handle);

		unsafe { self.ext.destroy_debug_utils_messenger(self.handle, None) }
	}
}

#[cfg(feature = "raii")]
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
	let message_id_name = crate::util::wrap_c_str(callback_data.p_message_id_name)
		.into_string()
		.unwrap();
	let message = crate::util::wrap_c_str(callback_data.p_message)
		.into_string()
		.unwrap();

	match severity {
		vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => {
			println!(
				"[{:?}]: {} ({}): {}",
				msg_type,
				message_id_name,
				&message_id_number.to_string(),
				message
			);
		}
		vk::DebugUtilsMessageSeverityFlagsEXT::INFO => {
			println!(
				"[{:?}]: {} ({}): {}",
				msg_type,
				message_id_name,
				&message_id_number.to_string(),
				message
			);
		}
		vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => {
			panic!(
				"[{:?}]: {} ({}): {}",
				msg_type,
				message_id_name,
				&message_id_number.to_string(),
				message
			);
		}
		vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => {
			panic!(
				"[{:?}]: {} ({}): {}",
				msg_type,
				message_id_name,
				&message_id_number.to_string(),
				message
			);
		}
		_ => {
			unimplemented!()
		}
	};

	vk::FALSE
}
