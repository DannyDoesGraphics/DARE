use std::sync::{Arc, RwLock, Weak};

use crate::primitives::MeshAsset;

pub trait Renderable {
    fn draw(
        &self,
        top_matrix: glam::Mat4,
        draw_context: &mut crate::assets::draw_context::DrawContext,
    );
}

pub struct Mesh {
    pub parent: Weak<RwLock<Mesh>>,
    pub children: Vec<Arc<RwLock<Mesh>>>,

    pub mesh: Arc<MeshAsset>,

    pub local_transform: glam::Mat4,
    pub world_transform: glam::Mat4,
}

impl Mesh {
    pub fn refresh_transform(&mut self, parent_matrix: glam::Mat4) {
        self.world_transform = parent_matrix * self.local_transform;
        for node in self.children.iter_mut() {
            let mut guard = node.write().unwrap();
            guard.refresh_transform(self.world_transform);
        }
    }
}

impl Renderable for Mesh {
    fn draw(
        &self,
        top_matrix: glam::Mat4,
        draw_context: &mut crate::assets::draw_context::DrawContext,
    ) {
        let node_matrix = top_matrix * self.world_transform;
        for surface in self.mesh.surfaces.iter() {
            draw_context
                .opaque_surfaces
                .push(crate::assets::render_object::RenderObject {
                    index_count: surface.count,
                    first_index: surface.start_index,
                    material: surface.material.clone(),
                    transform: node_matrix,
                    vertex_buffer: self.mesh.mesh_buffers.vertex_buffer.clone(),
                    index_buffer: self.mesh.mesh_buffers.index_buffer.clone(),
                })
        }

        // draw children
        for node in self.children.iter() {
            let guard = node.read().unwrap();
            guard.draw(top_matrix, draw_context);
        }
    }
}
