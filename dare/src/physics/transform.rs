#[derive(Clone, Debug, PartialEq)]
pub struct Transform {
    pub scale: glam::Vec3,
    pub rotation: glam::Quat,
    pub translation: glam::Vec3,
}

impl Transform {
    /// Quickly get the scale, rotation, and translation matrix
    pub fn get_transform_matrix(&self) -> glam::Mat4 {
        glam::Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}