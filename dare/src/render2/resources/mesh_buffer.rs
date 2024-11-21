use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use crate::prelude as dare;
use dare_containers::prelude as containers;

/// Defines a mesh buffer system
#[derive(becs::Resource)]
pub struct MeshBuffer<A: Allocator + 'static> {
    pub uploaded_hash: u64,
    pub growable_buffer: dare::render::util::GrowableBuffer<A>,
    pub mesh_container: containers::SlotMap<dare::engine::components::Mesh>,
    pub external_id_mapping: HashMap<becs::Entity, containers::Slot<dare::engine::components::Mesh>>,
}

impl<A: Allocator> Deref for MeshBuffer<A> {
    type Target = dare::render::util::GrowableBuffer<A>;

    fn deref(&self) -> &Self::Target {
        &self.growable_buffer
    }
}

impl<A: Allocator> DerefMut for MeshBuffer<A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.growable_buffer
    }
}

impl<A: Allocator + 'static> MeshBuffer<A> {
    pub async fn flush(&mut self, immediate_submit: &dare::render::util::ImmediateSubmit, asset_server: &dare::render::render_assets::RenderAssetsStorage<dare::render::components::RenderBuffer<GPUAllocatorImpl>>) {
        let surfaces = self.mesh_container.iter().flat_map(|(mesh, _)|
            dare::render::c::CSurface::from_surface(
                asset_server,
                mesh.surface.clone(),
                &mesh.transform.get_transform_matrix(),
            )
        ).collect::<Vec<_>>();
        let hash = {
            let mut hasher = std::hash::DefaultHasher::default();
            surfaces.hash(&mut hasher);
            hasher.finish()
        };
        if self.uploaded_hash != hash && !surfaces.is_empty() {
            self.growable_buffer.upload_to_buffer(
                immediate_submit,
                &surfaces,
            ).await.unwrap();
            self.uploaded_hash = hash;
        }
    }
}