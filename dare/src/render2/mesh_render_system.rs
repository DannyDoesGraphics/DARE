use crate::physics::prelude as physics;
use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;

pub fn mesh_render_system<A: Allocator + 'static>(
    mut query: becs::Query<(
        &super::mesh::Mesh<A>,
        Option<&physics::components::Transform>,
    )>,
    mut render_context: becs::Res<super::render_context::RenderContext>,
) {
    for (mesh, transform) in &mut query {}
}
