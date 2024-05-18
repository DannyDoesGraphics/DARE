use crate::traits::Destructible;
use anyhow::Result;
use ash::vk;
use std::ffi::CString;
use std::ptr;

/// Every resource in Vulkan is expected to have a lifetime + debuggable
pub trait Resource<'a>: Destructible + Sized {
    /// Necessary create info
    type CreateInfo: 'a;
    /// Type of underlyin where Self::CreateInfo<'a>: 'a;g VkObject the struct is representing
    type HandleType;

    /// Attempt to create a new resource given the [`Self::CreateInfo`] struct
    fn new(create_info: Self::CreateInfo) -> Result<Self>
    where
        Self: Sized;

    /// Get a reference to the underlying VkObject
    fn get_handle(&self) -> &Self::HandleType;

    /// Get a copy to the underlying VkObject
    fn handle(&self) -> Self::HandleType;

    /// Get underlying reference to the device the object belongs to
    fn get_device(&self) -> &crate::device::LogicalDevice;

    fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> Result<()>;

    /// Get the name of the resource
    fn get_name(&self) -> Option<&str>;
}

/// Because the naming process is effectively the same, we condense it down here
pub(crate) fn name_resource(
    debug_utils: &ash::ext::debug_utils::Device,
    raw_handle: u64,
    object_type: vk::ObjectType,
    name: &str,
) -> Result<()> {
    /// Set name of VkObject
    let name = CString::new(name)?;
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
