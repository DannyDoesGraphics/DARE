use bevy_ecs::prelude as becs;
use std::time::Instant;

#[derive(Debug, becs::Resource)]
pub struct DeltaTime {
    prev: Instant,
    delta: f32,
}

impl Default for DeltaTime {
    fn default() -> Self {
        Self {
            prev: Instant::now(),
            delta: 0.0,
        }
    }
}

impl DeltaTime {
    pub fn update(&mut self) {
        let now = Instant::now();
        let dt = self.prev.elapsed().as_secs_f32();
        self.prev = now;
        self.delta = dt;
    }

    pub fn get_delta(&self) -> f32 {
        self.delta
    }
}

pub fn delta_time_update(mut delta_time: becs::ResMut<'_, DeltaTime>) {
    delta_time.update();
}
