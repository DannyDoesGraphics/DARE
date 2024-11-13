use bevy_ecs::prelude::*;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, PartialEq, Component)]
pub struct Velocity(pub glam::Vec3);

impl Default for Velocity {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl Deref for Velocity {
    type Target = glam::Vec3;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Velocity {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
