use std::ptr;

use anyhow::Result;
use ash::vk;
use ash::vk::Handle;

pub use acceleration_structure_build_geometry_info::AccelerationStructureBuildGeometryInfo as BuildGeometryInfo;
pub use acceleration_structure_build_geometry_info::*;

use crate::resource::traits::{Nameable, Resource};
use crate::traits::{AsRaw, Destructible};
use crate::DagalError;

pub mod acceleration_structure_build_geometry_info;
#[derive(Debug)]
pub struct AccelerationStructure {
    device: crate::device::LogicalDevice,
    handle: vk::AccelerationStructureKHR,
    ty: vk::AccelerationStructureTypeKHR,
}

pub enum AccelerationStructureInfo<'a> {
    FromCI {
        ci: &'a vk::AccelerationStructureCreateInfoKHR<'a>,
        device: crate::device::LogicalDevice,
        name: Option<&'a str>,
    },
}

impl<'a> Resource<'a> for AccelerationStructure {
    type CreateInfo = AccelerationStructureInfo<'a>;
    fn new(create_info: Self::CreateInfo) -> Result<Self>
    where
        Self: Sized,
    {
        match create_info {
            AccelerationStructureInfo::FromCI { ci, device, name } => {
                if let Some(acceleration_structure_func) =
                    device.get_acceleration_structure().as_ref()
                {
                    let handle = unsafe {
                        acceleration_structure_func.create_acceleration_structure(ci, None)
                    }?;

                    #[cfg(feature = "log-lifetimes")]
                    tracing::trace!("Created VkAccelerationStructure {:p}", handle);

                    Ok(Self {
                        device,
                        handle,
                        ty: ci.ty,
                    })
                } else {
                    Err(anyhow::Error::from(DagalError::NoExtensionSupported))
                }
            }
        }
    }

    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }
}

impl AsRaw for AccelerationStructure {
    type RawType = vk::AccelerationStructureKHR;

    unsafe fn as_raw(&self) -> &Self::RawType {
        &self.handle
    }

    unsafe fn as_raw_mut(&mut self) -> &mut Self::RawType {
        &mut self.handle
    }

    unsafe fn raw(self) -> Self::RawType {
        self.handle
    }
}

impl AccelerationStructure {
    pub fn ty(&self) -> vk::AccelerationStructureTypeKHR {
        self.ty
    }
}

impl Nameable for AccelerationStructure {
    const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::ACCELERATION_STRUCTURE_KHR;

    fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> Result<()> {
        crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
        Ok(())
    }
}

impl Destructible for AccelerationStructure {
    fn destroy(&mut self) {
        unsafe {
            #[cfg(feature = "log-lifetimes")]
            tracing::trace!("Destroying VkAccelerationStructure {:p}", self.handle);
            self.device
                .get_acceleration_structure()
                .unwrap()
                .destroy_acceleration_structure(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl Drop for AccelerationStructure {
    fn drop(&mut self) {
        self.destroy();
    }
}
