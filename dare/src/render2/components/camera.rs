use crate::prelude as dare;
use crate::window::input::Input;
use bevy_ecs::prelude as becs;
use dagal::winit;
use dagal::winit::event::{ElementState, MouseButton};

#[derive(Debug, PartialEq, Copy, Clone, becs::Component, becs::Resource)]
pub struct Camera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub position: glam::Vec3,
    pub velocity: glam::Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub speed: f32,
    pub now_rotating: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            fov: 70.0,
            near: 0.1,
            far: 1000.0,
            position: glam::Vec3::new(0.0, 4.0, 0.0),
            velocity: Default::default(),
            pitch: 0.0,
            yaw: 75_f32.to_radians(),
            speed: 1.0,
            now_rotating: false,
        }
    }
}

impl Camera {
    pub fn process_key_event(&mut self, input: &winit::event::KeyEvent) {
        use dagal::winit::event::{ElementState, KeyEvent};
        use dagal::winit::keyboard::{KeyCode, PhysicalKey};
        let pressed_or_released_modifier: f32 = if input.state == ElementState::Pressed {
            1.0
        } else {
            0.0
        };
        match input {
            KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::KeyW),
                repeat: false,
                ..
            } => {
                self.velocity.z = pressed_or_released_modifier * -1.0;
            }
            KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::KeyS),
                repeat: false,
                ..
            } => {
                self.velocity.z = pressed_or_released_modifier * 1.0;
            }
            KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::KeyA),
                repeat: false,
                ..
            } => {
                self.velocity.x = pressed_or_released_modifier * -1.0;
            }
            KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::KeyD),
                repeat: false,
                ..
            } => {
                self.velocity.x = pressed_or_released_modifier * 1.0;
            }
            KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::KeyQ),
                repeat: false,
                ..
            } => {
                self.velocity.y = pressed_or_released_modifier * 1.0;
            }
            KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::KeyE),
                repeat: false,
                ..
            } => {
                self.velocity.y = pressed_or_released_modifier * -1.0;
            }
            KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::ArrowUp),
                state: ElementState::Pressed,
                ..
            } => {
                self.speed *= 1.2;
                self.speed = self.speed.max(1.0)
            }
            KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::ArrowDown),
                state: ElementState::Pressed,
                ..
            } => {
                self.speed *= 0.8;
                self.speed = self.speed.max(1.0);
            }
            _ => {}
        }
    }

    pub fn process_mouse_event(&mut self, dx: f32, dy: f32, dt: f32) {
        if self.now_rotating {
            self.yaw += dx * dt;
            self.pitch += dy * dt;
        }
    }

    pub fn process_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        match button {
            MouseButton::Left => self.now_rotating = state.is_pressed(),
            MouseButton::Right => {}
            MouseButton::Middle => {}
            MouseButton::Back => {}
            MouseButton::Forward => {}
            MouseButton::Other(_) => {}
        }
    }

    fn get_rotation_matrix(&self) -> glam::Mat4 {
        let pitch = glam::Quat::from_axis_angle(glam::Vec3::X, self.pitch);
        let yaw = glam::Quat::from_axis_angle(-glam::Vec3::Y, self.yaw);

        glam::Mat4::from_quat(yaw) * glam::Mat4::from_quat(pitch)
    }

    pub fn get_view_matrix(&self) -> glam::Mat4 {
        let camera_translation = glam::Mat4::from_translation(self.position);
        let camera_rotation = self.get_rotation_matrix();
        glam::Mat4::inverse(&(camera_translation * camera_rotation))
    }

    pub fn get_projection(&self, aspect_ratio: f32) -> glam::Mat4 {
        let mut proj = glam::Mat4::perspective_rh(self.fov, aspect_ratio, self.far, self.near);
        proj.y_axis.y *= -1.0;
        proj
    }

    pub fn update(&mut self, dt: f32) {
        let rot = self.get_rotation_matrix();
        let dp = self.velocity * dt;
        let dp = rot * glam::Vec4::from((dp, 0.0));
        self.position += glam::Vec3::new(dp.x, dp.y, dp.z) * self.speed;
    }
}

pub fn camera_system(
    mut camera: becs::ResMut<'_, Camera>,
    mut input: becs::ResMut<'_, dare::util::event::EventReceiver<dare::window::input::Input>>,
    dt: becs::ResMut<dare::render::systems::delta_time::DeltaTime>,
) {
    let dt = dt.get_delta();
    while let Some(input) = input.next() {
        match input {
            Input::KeyEvent(key) => camera.process_key_event(&key),
            Input::MouseButton { button, state } => camera.process_mouse_button(button, state),
            Input::MouseWheel(_) => {}
            Input::MouseDelta(delta) => camera.process_mouse_event(delta.x, delta.y, dt),
        }
    }
    camera.update(dt);
}
