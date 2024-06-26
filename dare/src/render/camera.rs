use std::collections::HashSet;

use dagal::winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Debug, Default)]
pub struct Camera {
    velocity: glam::Vec3,
    position: glam::Vec3,
    button_down: bool,
    pitch: f32,
    yaw: f32,
    previous_mouse: Option<glam::Vec2>,
    keys: HashSet<KeyCode>,
    pub speed: f32,
}

impl Camera {
    pub fn get_view_matrix(&self) -> glam::Mat4 {
        let translation_matrix = glam::Mat4::from_translation(self.position);
        let camera_rotation = self.get_rotation_matrix();
        glam::Mat4::inverse(&(translation_matrix * camera_rotation))
    }

    pub fn get_rotation_matrix(&self) -> glam::Mat4 {
        let pitch_rotation = glam::Quat::from_axis_angle(glam::Vec3::from([1.0, 0.0, 0.0]), self.pitch);
        let yaw_rotation = glam::Quat::from_axis_angle(glam::Vec3::from([0.0, -1.0, 0.0]), self.yaw);
        glam::Mat4::from_quat(yaw_rotation) * glam::Mat4::from_quat(pitch_rotation)
    }

    pub fn process_input(&mut self, input_key: PhysicalKey, pressed: bool) {
        if let PhysicalKey::Code(key_code) = input_key {
            if pressed {
                self.keys.insert(key_code);
            } else {
                self.keys.remove(&key_code);
            }
            self.update_velocity();
        }
    }

    fn update_velocity(&mut self) {
        let rotation = self.get_rotation_matrix();
        let forward = rotation.transform_vector3(glam::Vec3::new(0.0, 0.0, -1.0));
        let right = rotation.transform_vector3(glam::Vec3::new(1.0, 0.0, 0.0));
        let up = glam::Vec3::cross(forward, right);

        let mut direction = glam::Vec3::ZERO;

        if self.keys.contains(&KeyCode::KeyW) {
            direction += forward;
        }
        if self.keys.contains(&KeyCode::KeyS) {
            direction -= forward;
        }
        if self.keys.contains(&KeyCode::KeyA) {
            direction -= right;
        }
        if self.keys.contains(&KeyCode::KeyD) {
            direction += right;
        }
        if self.keys.contains(&KeyCode::KeyQ) {
            direction += up;
        }
        if self.keys.contains(&KeyCode::KeyE) {
            direction -= up;
        }

        if direction.length_squared() > 0.0 {
            direction = direction.normalize();
        }

        self.velocity = direction;
    }

    pub fn process_mouse_input(&mut self, pos: glam::Vec2, dt: f32) {
        if let Some(prev_pos) = self.previous_mouse {
            if self.button_down {
                let diff = pos - prev_pos;
                self.yaw += diff.x * dt;
                self.pitch += diff.y * dt;
            }
        }
        self.previous_mouse = Some(pos);
    }

    pub fn button_down(&mut self, down: bool) {
        self.button_down = down;
    }

    pub fn mouse_scrolled(&mut self, scroll_delta: f32, dt: f32) {
        if self.speed == 0.0 {
            self.speed += 1.0 + 2.0 * scroll_delta * dt;
        } else {
            self.speed *= 1.0 + 10.0 * scroll_delta * dt;
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.position += self.velocity * self.speed * dt;
    }
}