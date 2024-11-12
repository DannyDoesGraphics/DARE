use bevy_ecs::prelude::*;

#[derive(Clone, Debug, PartialEq, Component)]
pub struct Transform {
    pub scale: glam::Vec3,
    pub rotation: glam::Quat,
    pub translation: glam::Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            scale: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            translation: glam::Vec3::ZERO,
        }
    }
}

impl Transform {
    /// Quickly get the scale, rotation, and translation matrix
    pub fn get_transform_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn as_raw(&self) -> [f32; 16] {
        self.get_transform_matrix().transpose().to_cols_array()
    }
}
