use std::sync::{Arc, Weak};

use dagal::allocators::{Allocator, GPUAllocatorImpl};

use crate::render;
use crate::traits::ReprC;

#[derive(Debug, Clone)]
pub struct Mesh<A: Allocator = GPUAllocatorImpl> {
    name: Option<String>,
    pub translation: glam::Vec3,
    pub scale: glam::Vec3,
    pub rotation: glam::Quat,
    pub(crate) surface: Arc<render::Surface<A>>,
}

/// Same as a [`Mesh`], but it only holds [`Weak`] references to [`render::Surface`]
#[derive(Debug, Clone)]
pub struct WeakMesh<A: Allocator = GPUAllocatorImpl> {
    name: Option<String>,
    pub position: glam::Vec3,
    pub scale: glam::Vec3,
    pub rotation: glam::Quat,
    surface: Weak<render::Surface<A>>,
}

impl<A: Allocator> Mesh<A> {
    /// Get the underlying surfaces of a mesh
    pub fn get_surface(&self) -> &Arc<render::Surface<A>> {
        &self.surface
    }

    pub fn weak_mesh(&self) -> WeakMesh<A> {
        WeakMesh {
            name: self.name.clone(),
            position: self.translation,
            scale: self.scale,
            rotation: self.rotation,
            surface: Arc::downgrade(&self.surface),
        }
    }

    pub fn new(
        name: Option<String>,
        translation: glam::Vec3,
        scale: glam::Vec3,
        rotation: glam::Quat,
        surface: Arc<render::Surface<A>>,
    ) -> Self {
        Self {
            name,
            translation,
            scale,
            rotation,
            surface,
        }
    }

    pub fn as_c(&self) -> CMesh {
        let c_surface = self.surface.as_c();
        CMesh {
            material: c_surface.material,
            buffer_flags: c_surface.buffer_flags,
            vertices: c_surface.vertices,
            indices: c_surface.indices,
            normals: c_surface.normals,
            tangents: c_surface.tangents,
            uvs: c_surface.uvs,
            transform: glam::Mat4::from_scale_rotation_translation(self.translation, self.rotation, self.scale).transpose().to_cols_array(),
        }
    }
}

impl<A: Allocator> WeakMesh<A> {
    pub fn get_surfaces(&self) -> &Weak<render::Surface<A>> {
        &self.surface
    }

    pub fn as_mesh(&self) -> Option<Mesh<A>> {
        Some(Mesh {
            name: self.name.clone(),
            translation: self.position,
            scale: self.scale,
            rotation: self.rotation,
            surface: Weak::upgrade(&self.surface)?,
        })
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CMesh {
    pub material: u64,
    pub buffer_flags: u32,
    pub vertices: u64,
    pub indices: u64,
    pub normals: u64,
    pub tangents: u64,
    pub uvs: u64,
    pub transform: [f32; 16],
}