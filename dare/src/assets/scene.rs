use std::ffi::c_void;
use std::sync::{Arc, Weak};

use anyhow::Result;

use dagal::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use dagal::resource;
use dagal::util::{ImmediateSubmit, Slot};

use crate::render::{CMesh, Mesh, WeakMesh};
use crate::render::deferred_deletion::{DeferredDeletion, DeletionEntry};
use crate::render::growable_buffer::GrowableBuffer;

/// Describes a collection of meshes and objects
#[derive(Debug)]
pub struct Scene<A: Allocator = GPUAllocatorImpl> {
    pub meshes: DeferredDeletion<WeakMesh<A>>,
    pub mesh_info_buffer: GrowableBuffer<A>,
    pub surfaces: Vec<Weak<crate::render::Surface<A>>>,
    pub acceleration_structures: DeferredDeletion<resource::AccelerationStructure>,
}

/// Wraps over a [`Mesh`] to include extra data about it such as acceleration structures
pub struct SceneMesh<A: Allocator = GPUAllocatorImpl> {
    mesh: Mesh<A>,
    acceleration_structure: Option<resource::acceleration_structure::AccelerationStructure>,
}

impl<A: Allocator> Scene<A> {
    /// Insert surface
    pub fn insert_surfaces(&mut self, surfaces: &[Arc<crate::render::Surface<A>>]) {
        self.surfaces.append(&mut surfaces.iter().map(Arc::downgrade).collect());
    }

    /// Inserts meshes
    pub fn insert_meshes(&mut self, meshes: &[Mesh<A>]) -> Vec<Slot<DeletionEntry<WeakMesh<A>>>> {
        meshes.iter().map(|mesh| {
            self.meshes.insert(mesh.weak_mesh(), 16)
        }).collect()
    }

    /// Upload all mesh info
    pub fn upload_mesh_info(&mut self, allocator: &mut ArcAllocator<A>, immediate_submit: &mut ImmediateSubmit) -> Result<()> {
        let c_meshes: Vec<Option<CMesh>> = self.meshes.deferred_elements.iter().map(|mesh| {
            mesh.data.as_ref().map(|mesh| mesh.element.as_mesh().unwrap().as_c())
        }).collect();
        self.mesh_info_buffer.update_buffer(allocator, immediate_submit, (c_meshes.len() * std::mem::size_of::<CMesh>()) as vk::DeviceSize)?;
        let ptr = self.mesh_info_buffer.get_handle().mapped_ptr().unwrap().as_ptr();
        for (index, c_mesh) in c_meshes.into_iter().enumerate() {
            if let Some(c_mesh) = c_mesh {
                unsafe {
                    ptr.byte_add(index * std::mem::size_of::<CMesh>()).copy_from(
                        &c_mesh as *const _ as *const c_void,
                        std::mem::size_of::<CMesh>()
                    );
                }
            }
        }
        Ok(())
    }

    pub fn get_scene_meshes(&self) -> Vec<Option<DeletionEntry<Mesh<A>>>> {
        self.meshes.deferred_elements.data().iter()
            .map(|deletion_entry| {
                deletion_entry.data.as_ref().and_then(|deletion_entry| {
                    deletion_entry.element.as_mesh().map(|mesh| {
                        DeletionEntry::new(mesh, deletion_entry.ttl(), deletion_entry.last_used())
                    })
                })
            })
            .collect()
    }
    pub fn keep_meshes_alive(&mut self, meshes: &[Slot<DeletionEntry<Mesh<A>>>]) -> Result<()> {
        let mut weak_meshes: Vec<Slot<DeletionEntry<WeakMesh<A>>>> = meshes
            .iter()
            .map(|mesh| {
                Slot::<DeletionEntry<WeakMesh<A>>>::new(
                    mesh.id(),
                    Some(mesh.generation())
                )
            }).collect();
        self.keep_meshes_alive_weak(weak_meshes.as_mut_slice())?;
        Ok(())
    }

    pub fn keep_meshes_alive_weak(&mut self, meshes: &mut [Slot<DeletionEntry<WeakMesh<A>>>]) -> Result<()> {
        for weak_mesh in meshes {
            self.meshes.update(weak_mesh)?;
        }
        Ok(())
    }

    pub fn new(device: dagal::device::LogicalDevice, allocator: &mut ArcAllocator<A>) -> Result<Self> {
        Ok(Self {
            meshes: Default::default(),
            mesh_info_buffer: GrowableBuffer::new(
                dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                    device,
                    allocator,
                    size: std::mem::size_of::<CMesh>() as vk::DeviceSize,
                    memory_type: MemoryLocation::CpuToGpu,
                    usage_flags: vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER
                        | vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::TRANSFER_DST,
                }
            )?,
            surfaces: Vec::new(),
            acceleration_structures: Vec::new(),
        })
    }
}
