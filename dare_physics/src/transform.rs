use bevy_ecs::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct Transform {
    pub scale: glam::Vec3,
    pub rotation: glam::Quat,
    pub translation: glam::Vec3,
}

impl Eq for Transform {}

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
    pub fn get_transform_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    #[allow(dead_code)]
    pub fn as_raw(&self) -> [f32; 16] {
        self.get_transform_matrix().to_cols_array()
    }
}

impl From<glam::Mat4> for Transform {
    fn from(value: glam::Mat4) -> Self {
        let (scale, rotation, translation) = value.to_scale_rotation_translation();
        Self {
            scale,
            rotation,
            translation,
        }
    }
}
