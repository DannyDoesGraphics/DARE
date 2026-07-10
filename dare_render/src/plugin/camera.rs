use bevy_ecs::prelude::*;
use dare_ecs::{App, ExtractPlugin, Plugin, SubAppMainLabel};
use dare_physics::Transform;
use glam::{Mat4, Quat, Vec3};

use crate::plugin::RenderSubAppLabel;

/// Systems which author the [`Camera`] pose. Readers should run `.after(CameraUpdate)`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CameraUpdate;

/// The scene's single camera: a pose plus a lens. `scale` is ignored by [`Camera::view`].
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub transform: Transform,
    /// Vertical field of view, radians.
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            transform: Transform {
                scale: Vec3::ONE,
                rotation: Quat::IDENTITY,
                translation: Vec3::new(0.0, 0.0, 3.0),
            },
            fov_y: 70.0_f32.to_radians(),
            near: 0.1,
            far: 10_000.0,
        }
    }
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        self.transform.rotation * Vec3::NEG_Z
    }

    pub fn right(&self) -> Vec3 {
        self.transform.rotation * Vec3::X
    }

    pub fn up(&self) -> Vec3 {
        self.transform.rotation * Vec3::Y
    }

    /// World space -> right-handed, Y-up view space.
    pub fn view(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.transform.rotation, self.transform.translation)
            .inverse()
    }

    /// View space -> Vulkan NDC (Z in `[0, 1]`, Y-down).
    pub fn projection(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::vulkan::perspective(self.fov_y, aspect, self.near, self.far)
    }
}

/// Owns the [`Camera`] resource and extracts it to the render world.
#[derive(Default)]
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().init_resource::<Camera>();

        app.get_sub_app_mut::<RenderSubAppLabel>()
            .expect("CameraPlugin requires the render sub-app")
            .world_mut()
            .init_resource::<Camera>();

        app.add_plugin(
            ExtractPlugin::<Camera, RenderSubAppLabel, SubAppMainLabel>::from_cloneable_resource(),
        );
    }
}
