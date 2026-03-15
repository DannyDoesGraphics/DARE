use std::ops::{Deref, DerefMut};

use dare_ecs::Plugin;
use bevy_ecs::{entity::EntityHashMap, prelude::*};

#[derive(Debug, Resource)]
pub(crate) struct EntityWorldSync {
    mapping: EntityHashMap<Entity>,
}
impl Deref for EntityWorldSync {
    type Target = EntityHashMap<Entity>;
    
    fn deref(&self) -> &Self::Target {
        &self.mapping
    }
}
impl DerefMut for EntityWorldSync {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.mapping
    }
}

#[derive(Default, Debug)]
pub(crate) struct EntityWorldSyncPlugin {}

impl Plugin for EntityWorldSyncPlugin {
    fn build(&self, world: &mut dare_ecs::App) {
        // check if resource already exists
        if world.world().contains_resource::<EntityWorldSync>() {
            return;
        } else {
            world.world_mut().insert_resource(EntityWorldSync {
                mapping: EntityHashMap::new(),
            });
        }
    }
}