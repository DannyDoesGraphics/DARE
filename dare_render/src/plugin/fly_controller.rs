use std::time::Instant;

use bevy_ecs::prelude::*;
use dagal::winit::event::{MouseButton, MouseScrollDelta};
use dagal::winit::keyboard::KeyCode;
use dare_ecs::{App, AppStage, Plugin};
use dare_window::{Input, InputLog};
use glam::{Quat, Vec2, Vec3};

use crate::plugin::{Camera, CameraPlugin, CameraUpdate};

const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 1e-3;
const SPEED_STEP: f32 = 1.1;
const MIN_SPEED: f32 = 0.05;
const MAX_SPEED: f32 = 500.0;
const MAX_DELTA: f32 = 0.1;

/// Free-look controller state.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct FlyController {
    /// Radians, rotation about world +Y.
    pub yaw: f32,
    /// Radians, clamped to +-[`PITCH_LIMIT`].
    pub pitch: f32,
    /// ms^-1, adjusted by the scroll wheel.
    pub speed: f32,
    /// Radians/pixel of mouse movement.
    pub sensitivity: f32,
}

impl Default for FlyController {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            speed: 3.0,
            sensitivity: 0.002,
        }
    }
}

impl FlyController {
    fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(self.pitch)
    }
}

#[derive(Resource, Default)]
struct FlyClock {
    last: Option<Instant>,
}

/// Drives the [`Camera`] pose from input: mouse look, `WASD` + `Q`/`E`, scroll for speed.
#[derive(Default)]
pub struct FlyControllerPlugin;

impl Plugin for FlyControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(CameraPlugin);

        app.world_mut().init_resource::<FlyController>();
        app.world_mut().init_resource::<FlyClock>();

        app.schedule_scope(|schedule| {
            schedule.add_systems(fly_camera.in_set(AppStage::Update).in_set(CameraUpdate));
        });
    }
}

fn fly_camera(
    input: Res<InputLog>,
    mut camera: ResMut<Camera>,
    mut controller: ResMut<FlyController>,
    mut clock: ResMut<FlyClock>,
) {
    let now = Instant::now();
    let dt = clock
        .last
        .map(|last| (now - last).as_secs_f32().min(MAX_DELTA))
        .unwrap_or(0.0);
    clock.last = Some(now);

    let (look, scroll) = accumulate(input.events());

    if scroll != 0.0 {
        controller.speed = (controller.speed * SPEED_STEP.powf(scroll)).clamp(MIN_SPEED, MAX_SPEED);
    }

    if look != Vec2::ZERO && input.is_mouse_pressed(MouseButton::Left) {
        controller.yaw -= look.x * controller.sensitivity;
        controller.pitch =
            (controller.pitch - look.y * controller.sensitivity).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        camera.transform.rotation = controller.rotation();
    }

    let (forward, right) = (camera.forward(), camera.right());
    let mut direction = Vec3::ZERO;
    if input.is_key_pressed(KeyCode::KeyW) {
        direction += forward;
    }
    if input.is_key_pressed(KeyCode::KeyS) {
        direction -= forward;
    }
    if input.is_key_pressed(KeyCode::KeyD) {
        direction += right;
    }
    if input.is_key_pressed(KeyCode::KeyA) {
        direction -= right;
    }
    if input.is_key_pressed(KeyCode::KeyQ) {
        direction += Vec3::Y;
    }
    if input.is_key_pressed(KeyCode::KeyE) {
        direction -= Vec3::Y;
    }

    if direction != Vec3::ZERO {
        let velocity = direction.normalize() * controller.speed * dt;
        camera.transform.translation += velocity;
    }
}

fn accumulate(events: &[Input]) -> (Vec2, f32) {
    let mut look = Vec2::ZERO;
    let mut scroll = 0.0;
    for event in events {
        match event {
            Input::MouseDelta(delta) => look += *delta,
            Input::MouseWheel(delta) => {
                scroll += match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 120.0,
                };
            }
            _ => {}
        }
    }
    (look, scroll)
}
