use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
pub mod plugin;

/// A simple application to emulate what Bevy does.
pub struct App {
    world: World,
}

/// Scheduler by default used
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct InternalSchedule;

#[derive(SystemSet, Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum AppStage {
    First,
    PreUpdate,
    Update,
    PostUpdate,
    Last,
}

impl App {
    pub fn new() -> Self {
        let mut world = World::new();
        let mut schedule = Schedule::new(InternalSchedule);
        schedule.configure_sets(
            (
                AppStage::First,
                AppStage::PreUpdate,
                AppStage::Update,
                AppStage::PostUpdate,
                AppStage::Last,
            )
                .chain(),
        );
        assert!(
            world
                .get_resource_or_init::<Schedules>()
                .insert(schedule)
                .is_none()
        );
        Self { world }
    }

    #[inline]
    pub fn world(&self) -> &World {
        &self.world
    }
    #[inline]
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
    #[inline]
    pub fn schedule(&self) -> &Schedule {
        &self
            .world
            .get_resource::<Schedules>()
            .unwrap()
            .get(InternalSchedule)
            .unwrap()
    }
    pub fn schedule_scope<O, F: FnOnce(&mut Schedule) -> O>(&mut self, f: F) -> O {
        self.world
            .get_resource_mut::<Schedules>()
            .and_then(|mut schedules| {
                schedules
                    .get_mut(InternalSchedule)
                    .map(|schedule| f(schedule))
            })
            .unwrap()
    }

    /// Tick the app
    pub fn tick(&mut self) {
        self.world.run_schedule(InternalSchedule);
        self.world.clear_trackers();
    }
    
    /// Add a plugin to application
    pub fn add_plugins<T: plugin::Plugin>(mut self, plugin: T) -> Self {
        plugin.build(&mut self);
        self
    }
}
