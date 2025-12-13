use crate::prelude::physics;
use bevy_ecs::prelude::*;

fn cull_system(query: Query<&physics::components::Transform>) {
    for transform in query {}
}
