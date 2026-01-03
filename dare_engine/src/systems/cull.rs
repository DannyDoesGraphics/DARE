use bevy_ecs::prelude::*;

/// CPU culling for now
/// TODO: Implement GPU culler
pub fn cull(
    cameras: Query<(&crate::components::Camera, &dare_physics::Transform)>,
    mesh: Query<(
        &dare_assets::MeshHandle,
        &dare_physics::BoundingBox,
        &dare_physics::Transform,
    )>,
) {
    // TODO: care about culling later, for now we will concern ourselves with world
    /*
    for (camera, camera_transform) in cameras {
        let camera_frustum = glam::Mat4::perspective_
        mesh.par_iter().for_each(|(_, bounding_box, physics)| {
            
        });
    }
    */
    
}
