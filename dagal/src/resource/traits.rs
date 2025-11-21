use std::ffi::CString;
use std::hash::Hash;
use std::ptr;

use anyhow::Result;
use ash::vk;

use crate::traits::AsRaw;

/// Every resource in Vulkan is expected to have a lifetime + debuggable
pub trait Resource: Hash + Sized + AsRaw {
    /// Necessary create info
    type CreateInfo<'a>: 'a;
    /// Attempt to create a new resource given the [`Self::CreateInfo`] struct
    fn new(create_info: Self::CreateInfo<'_>) -> Result<Self, crate::DagalError>
    where
        Self: Sized;
    /// Get underlying reference to the device the object belongs to
    fn get_device(&self) -> &crate::device::LogicalDevice;
}

/// A struct which can have a name applied onto it
pub trait Nameable {
    const OBJECT_TYPE: vk::ObjectType;

    /// Set the name of the resource
    fn set_name(
        &mut self,
        debug_utils: &ash::ext::debug_utils::Device,
        name: &str,
    ) -> Result<(), crate::DagalError>;
}

pub(crate) fn name_nameable<T: Nameable>(
    debug_utils: &ash::ext::debug_utils::Device,
    raw_handle: u64,
    name: &str,
) -> Result<(), crate::DagalError> {
    name_resource(debug_utils, raw_handle, T::OBJECT_TYPE, name)
}

pub(crate) fn update_name<T: Resource + Nameable>(
    resource: &mut T,
    name: Option<&str>,
) -> Option<Result<(), crate::DagalError>> {
    if let Some(name) = name {
        if let Some(debug_utils) = resource.get_device().clone().get_debug_utils() {
            return Some(resource.set_name(debug_utils, name));
        }
    }
    None
}

/// Because the naming process is effectively the same, we condense it down here
pub(crate) fn name_resource(
    debug_utils: &ash::ext::debug_utils::Device,
    raw_handle: u64,
    object_type: vk::ObjectType,
    name: &str,
) -> Result<(), crate::DagalError> {
    let name = CString::new(name).map_err(|_| crate::DagalError::StringContainsNull)?;
    unsafe {
        debug_utils.set_debug_utils_object_name(&vk::DebugUtilsObjectNameInfoEXT {
            s_type: vk::StructureType::DEBUG_UTILS_OBJECT_NAME_INFO_EXT,
            p_next: ptr::null(),
            object_type,
            object_handle: raw_handle,
            p_object_name: name.as_ptr(),
            _marker: Default::default(),
        })
    }?;
    Ok(())
}
