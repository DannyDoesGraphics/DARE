use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_ecs::system::ScheduleSystem;

/// A simple application to emulate what Bevy does.
pub struct App {
    world: World,
}

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

        fn update_system(world: &mut World) {
            world.clear_trackers();
        }
        assert!(
            world
                .get_resource_or_init::<Schedules>()
                .insert(schedule)
                .is_none()
        );

        let mut schedules = world.get_resource_mut::<Schedules>().unwrap();
        let schedule = schedules.get_mut(InternalSchedule).unwrap();
        schedule.add_systems(update_system.in_set(AppStage::Last));

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
    }
}
