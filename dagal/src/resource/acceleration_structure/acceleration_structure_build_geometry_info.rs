use std::ptr;

use ash::vk;

use crate::traits::AsRaw;

#[derive(Debug, Copy, Clone)]
pub struct AccelerationStructureBuildGeometryInfo<'a> {
    handle: vk::AccelerationStructureBuildGeometryInfoKHR<'a>,
}

impl<'a> Default for AccelerationStructureBuildGeometryInfo<'a> {
    fn default() -> Self {
        Self {
            handle: vk::AccelerationStructureBuildGeometryInfoKHR {
                s_type: vk::StructureType::ACCELERATION_STRUCTURE_BUILD_GEOMETRY_INFO_KHR,
                p_next: ptr::null(),
                ..Default::default()
            },
        }
    }
}

impl<'a> AccelerationStructureBuildGeometryInfo<'a> {
    pub fn ty(mut self, ty: vk::AccelerationStructureTypeKHR) -> Self {
        self.handle.ty = ty;
        self
    }

    pub fn flags(mut self, flags: vk::BuildAccelerationStructureFlagsKHR) -> Self {
        self.handle.flags = flags;
        self
    }

    pub fn mode(mut self, mode: vk::BuildAccelerationStructureModeKHR) -> Self {
        self.handle.mode = mode;
        self
    }

    pub fn src_acceleration_structure(
        mut self,
        accel: Option<vk::AccelerationStructureKHR>,
    ) -> Self {
        self.handle.src_acceleration_structure =
            accel.unwrap_or(vk::AccelerationStructureKHR::null());
        self
    }

    pub fn dst_acceleration_structure(
        mut self,
        accel: Option<vk::AccelerationStructureKHR>,
    ) -> Self {
        self.handle.dst_acceleration_structure =
            accel.unwrap_or(vk::AccelerationStructureKHR::null());
        self
    }

    pub fn p_geometries(mut self, geometries: &'a [vk::AccelerationStructureGeometryKHR]) -> Self {
        self.handle.p_geometries = geometries.as_ptr();
        self.handle.geometry_count = geometries.len() as u32;
        self
    }

    pub fn scratch_data(mut self, data: vk::DeviceOrHostAddressKHR) -> Self {
        self.handle.scratch_data = data;
        self
    }
}

impl<'a> AsRaw for AccelerationStructureBuildGeometryInfo<'a> {
    type RawType = vk::AccelerationStructureBuildGeometryInfoKHR<'a>;

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
