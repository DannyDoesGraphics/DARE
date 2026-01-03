use bevy_ecs::prelude::*;

#[derive(Debug, Default, PartialEq, Clone, Component)]
pub struct Plane {
    pub normal: glam::Vec3,
    pub distance: f32,
}

impl Plane {
    /// Ensure the normal is... actually a normal
    pub fn normalize(self) -> Self {
        let length = self.normal.length();
        if length > 0.0 {
            Self {
                normal: self.normal / length,
                distance: self.distance * length
            }
        } else {
            self
        }
    }
}