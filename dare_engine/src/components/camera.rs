pub use bevy_ecs::prelude::*;

#[derive(Debug, PartialEq, Clone, Component)]
pub struct Camera {
    pub fov: f64,
    pub yaw: f32,
    pub pitch: f32,
}

impl dare_extract::Project for Camera {
    type Extracted = Self;
    type Filter = ();

    fn extract(&self) -> Self::Extracted {
        self.clone()
    }

    fn consume(extract: Self::Extracted) -> Self {
        extract
    }
}
