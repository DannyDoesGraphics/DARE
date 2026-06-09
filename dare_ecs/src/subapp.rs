use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ExecutorKind, IntoScheduleConfigs, ScheduleLabel};
use bevy_ecs::system::ScheduleSystem;

/// A unique label identifier for each [`SubApp`]
pub trait SubAppLabel: Send + Sync + 'static {}

/// The default main label which always exists for every app.
pub struct SubAppMainLabel;
impl SubAppLabel for SubAppMainLabel {}

#[derive(Debug)]
pub struct SubApp {
    world: World,
}

impl Default for SubApp {
    fn default() -> Self {
        Self::new()
    }
}

impl SubApp {
    /// Each subapp has a [`crate::InternalSchedule`] which is ticked by [`crate::SubApp::tick`]
    ///
    /// By default, a flush system is ran in [`AppStage::Last`] which ensures all commands are processed at the end.
    pub fn new() -> Self {
        let mut world = World::new();
        let mut schedule = Schedule::new(crate::InternalSchedule);
        schedule.configure_sets(
            (
                crate::AppStage::First,
                crate::AppStage::PreUpdate,
                crate::AppStage::Update,
                crate::AppStage::PostUpdate,
                crate::AppStage::Last,
            )
                .chain(),
        );
        schedule.set_executor_kind(ExecutorKind::MultiThreaded);

        world.get_resource_or_init::<Schedules>().insert(schedule);
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
        self.world
            .get_resource::<Schedules>()
            .unwrap()
            .get(crate::InternalSchedule)
            .unwrap()
    }

    /// Fetches the internal default schedule
    pub fn schedule_scope<O, F: FnOnce(&mut Schedule) -> O>(&mut self, f: F) -> O {
        self.schedule_scope_for(crate::InternalSchedule, f)
    }

    /// Fetches a schedule by label
    pub fn schedule_scope_for<L: ScheduleLabel, O, F: FnOnce(&mut Schedule) -> O>(
        &mut self,
        label: L,
        f: F,
    ) -> O {
        self.world
            .get_resource_mut::<Schedules>()
            .and_then(|mut schedules| schedules.get_mut(label).map(f))
            .unwrap()
    }

    /// Adds systems to the schedule
    pub fn add_systems<M>(
        &mut self,
        label: impl ScheduleLabel,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.world
            .get_resource_or_init::<Schedules>()
            .add_systems(label, systems);
        self
    }

    /// Runs a schedule by label without clearing change trackers.
    pub fn run_schedule(&mut self, label: impl ScheduleLabel) {
        self.world.run_schedule(label);
    }

    /// Tick the subapp
    pub fn tick(&mut self) {
        self.world.run_schedule(crate::InternalSchedule);
        // Flush on the ticking thread; an exclusive `|world|` system in an MT schedule can run on a worker.
        self.world.flush();
        self.world.clear_trackers();
    }
}
