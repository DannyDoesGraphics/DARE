use std::ffi::c_void;
use std::ptr;
use std::sync::Weak;

use dagal::allocators::{Allocator, ArcAllocator, MemoryLocation};
use dagal::ash::vk;
use dagal::descriptor::GPUResourceTable;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;

use crate::render::AlphaMode;

pub fn as_from_scene<A: Allocator + 'static>(gpu_rt: GPUResourceTable<A>, allocator: ArcAllocator<A>, scene: &crate::assets::scene::Scene) {
    let identity_matrix = glam::Mat4::IDENTITY.transpose().to_cols_array();
    let acceleration_structures = scene.surfaces.iter().map(|surface| {
        if let Some(surface) = Weak::upgrade(surface) {
            let gpu_rt = gpu_rt.clone();
            let mut allocator = allocator.clone();
            tokio::spawn(async move {
                let triangle_count: u32 = surface.index_count().div_ceil(3);
                let geometry: vk::AccelerationStructureGeometryKHR =
                    vk::AccelerationStructureGeometryKHR {
                        s_type: vk::StructureType::ACCELERATION_STRUCTURE_GEOMETRY_KHR,
                        p_next: ptr::null(),
                        geometry_type: vk::GeometryTypeKHR::TRIANGLES,
                        geometry: vk::AccelerationStructureGeometryDataKHR {
                            triangles: vk::AccelerationStructureGeometryTrianglesDataKHR {
                                s_type: vk::StructureType::ACCELERATION_STRUCTURE_GEOMETRY_TRIANGLES_DATA_KHR,
                                p_next: ptr::null(),
                                vertex_format: vk::Format::R32G32B32_SFLOAT,
                                vertex_data: vk::DeviceOrHostAddressConstKHR {
                                    device_address: surface.get_gpu_rt().get_bda(surface.get_vertex_buffer())?
                                },
                                vertex_stride: (std::mem::size_of::<f32>() * 3) as vk::DeviceSize,
                                max_vertex: (surface.vertex_count() - 1).max(surface.index_count() - 1),
                                index_type: vk::IndexType::UINT32,
                                index_data: vk::DeviceOrHostAddressConstKHR {
                                    device_address: surface.get_gpu_rt().get_bda(surface.get_index_buffer())?
                                },
                                transform_data: vk::DeviceOrHostAddressConstKHR {
                                    host_address: identity_matrix.as_ptr() as *const c_void, 
                                },
                                _marker: Default::default(),
                            }
                        },
                        flags: match surface.material().alpha_mode() {
                            AlphaMode::Opaque => {
                                vk::GeometryFlagsKHR::OPAQUE
                            },
                            AlphaMode::Blend | AlphaMode::Mask(_) => {
                                vk::GeometryFlagsKHR::empty()
                            }
                        },
                        _marker: Default::default(),
                    };
                let binding = [geometry];
                let build_info = dagal::resource::acceleration_structure::BuildGeometryInfo::default()
                    .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
                    .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
                    .p_geometries(&binding);
                if let Some(fnc) = gpu_rt.get_device().get_acceleration_structure() {
                    let mut size_info = vk::AccelerationStructureBuildSizesInfoKHR {
                        s_type: vk::StructureType::ACCELERATION_STRUCTURE_BUILD_SIZES_INFO_KHR,
                        p_next: ptr::null(),
                        ..Default::default()
                    };
                    unsafe {
                        fnc.get_acceleration_structure_build_sizes(
                            vk::AccelerationStructureBuildTypeKHR::DEVICE,
                            build_info.as_raw(),
                            &[triangle_count],
                            &mut size_info,
                        );
                    }
                    let scratch_buffer = dagal::resource::Buffer::new(
                        dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                            device: gpu_rt.get_device().clone(),
                            allocator: &mut allocator,
                            size: size_info.build_scratch_size,
                            memory_type: MemoryLocation::GpuOnly,
                            usage_flags: vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER,
                        }
                    )?;
                    let accel_buffer = dagal::resource::Buffer::new(
                        dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                            device: gpu_rt.get_device().clone(),
                            allocator: &mut allocator,
                            size: size_info.acceleration_structure_size,
                            memory_type: MemoryLocation::GpuOnly,
                            usage_flags: vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR,
                        }
                    )?;
                    let build_info = build_info.scratch_data(vk::DeviceOrHostAddressKHR {
                        device_address: scratch_buffer.address()
                    });
                    let accel_structure = dagal::resource::AccelerationStructure::new(
                        dagal::resource::AccelerationStructureInfo::FromCI {
                            ci: &vk::AccelerationStructureCreateInfoKHR {
                                s_type: vk::StructureType::ACCELERATION_STRUCTURE_CREATE_INFO_KHR,
                                p_next: ptr::null(),
                                create_flags: vk::AccelerationStructureCreateFlagsKHR::empty(),
                                buffer: unsafe { *accel_buffer.as_raw() },
                                offset: 0,
                                size: accel_buffer.get_size(),
                                ty: unsafe { build_info.as_raw().ty },
                                device_address: 0,
                                _marker: Default::default(),
                            },
                            device: gpu_rt.get_device().clone(),
                            name: surface.name(),
                        }
                    );
                }
                Ok::<(), anyhow::Error>(())
            });
        }
    });
}