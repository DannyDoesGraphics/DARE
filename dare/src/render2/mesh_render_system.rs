use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;
use crate::physics::prelude as physics;

pub fn mesh_render_system<A: Allocator + 'static>(
    mut query: becs::Query<(&super::mesh::Mesh<A>, Option<&physics::components::Transform>)>
) {
    for (mesh, transform) in &mut query {
    }
}