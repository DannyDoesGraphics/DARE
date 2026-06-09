use std::{any::TypeId, collections::HashMap};

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::ScheduleLabel;
pub mod extract;
mod plugin;
mod subapp;
pub mod smol_plugin;
pub use smol_plugin::{SmolExecutor, SmolExecutorHandle, SmolPlugin};
pub use extract::ExtractPlugin;
pub use extract::{Project, ProjectPlugin};
pub use plugin::*;
pub use subapp::*;

/// Not all subapps are connected to the main thread; some are pipelined or async.
pub enum SubAppHandle {
    /// A subapp that runs completely in a separate thread that is non-blocking
    Async {
        thread: std::thread::JoinHandle<()>,
        signal: crossbeam_channel::Sender<()>,
    },
    /// A subapp that exists on the main thread
    Sync(SubApp),
}
impl SubAppHandle {
    pub fn unwrap_sync(&self) -> &SubApp {
        match self {
            Self::Async { .. } => panic!("Expected Sync, got Async"),
            Self::Sync(subapp) => subapp,
        }
    }

    pub fn unwrap_mut_sync(&mut self) -> &mut SubApp {
        match self {
            Self::Async { .. } => panic!("Expected Sync, got Async"),
            Self::Sync(subapp) => subapp,
        }
    }
}

pub struct App {
    subapps: HashMap<TypeId, SubAppHandle>,
    plugin_registry: Option<plugin::PluginRegistry>,
    runner: Option<Box<dyn FnOnce(App)>>,
}
unsafe impl Send for App {}

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

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let mut subapps: HashMap<TypeId, SubAppHandle> = HashMap::new();
        subapps.insert(
            TypeId::of::<SubAppMainLabel>(),
            SubAppHandle::Sync(SubApp::new()),
        );
        Self {
            subapps,
            plugin_registry: Some(plugin::PluginRegistry::new()),
            runner: None,
        }
    }

    pub fn insert_sub_app<T: SubAppLabel>(&mut self, sub_app: SubApp) -> &mut Self {
        self.subapps
            .insert(TypeId::of::<T>(), SubAppHandle::Sync(sub_app));
        self
    }

    pub fn remove_sub_app<T: SubAppLabel>(&mut self) -> Option<SubApp> {
        self.subapps
            .remove(&TypeId::of::<T>())
            .and_then(|handle| match handle {
                SubAppHandle::Sync(sub_app) => Some(sub_app),
                SubAppHandle::Async { .. } => None,
            })
    }

    pub fn get_sub_app<T: SubAppLabel>(&self) -> Option<&SubApp> {
        self.subapps
            .get(&TypeId::of::<T>())
            .and_then(|handle| match handle {
                SubAppHandle::Sync(sub_app) => Some(sub_app),
                SubAppHandle::Async { .. } => None,
            })
    }

    pub fn get_sub_app_mut<T: SubAppLabel>(&mut self) -> Option<&mut SubApp> {
        self.subapps
            .get_mut(&TypeId::of::<T>())
            .and_then(|handle| match handle {
                SubAppHandle::Sync(sub_app) => Some(sub_app),
                SubAppHandle::Async { .. } => None,
            })
    }

    pub fn set_sub_app_handle<T: SubAppLabel>(&mut self, handle: SubAppHandle) -> &mut Self {
        self.subapps.insert(TypeId::of::<T>(), handle);
        self
    }

    pub fn set_runner(&mut self, f: Box<dyn FnOnce(App)>) -> &mut Self {
        self.runner = Some(f);
        self
    }

    pub fn add_plugin<T: Plugin + 'static>(&mut self, plugin: T) -> &mut Self {
        if self.plugin_registry.as_ref().unwrap().contains::<T>() {
            return self;
        }
        plugin.build(self);
        self.plugin_registry.as_mut().unwrap().register(plugin);
        self
    }

    pub fn finish_and_cleanup(&mut self) {
        let mut registry = self.plugin_registry.take().unwrap();
        registry.finish(self);
        registry.cleanup(self);
        self.plugin_registry = Some(registry);
    }

    pub fn run(mut self) {
        self.finish_and_cleanup();
        if let Some(runner) = self.runner.take() {
            runner(self);
        }
    }

    pub fn contains_plugin<P: Plugin + 'static>(&self) -> bool {
        self.plugin_registry.as_ref().unwrap().contains::<P>()
    }

    #[inline]
    pub fn world(&self) -> &World {
        self.get_sub_app::<SubAppMainLabel>().unwrap().world()
    }
    #[inline]
    pub fn world_mut(&mut self) -> &mut World {
        self.get_sub_app_mut::<SubAppMainLabel>()
            .unwrap()
            .world_mut()
    }
    #[inline]
    pub fn schedule(&self) -> &Schedule {
        self.get_sub_app::<SubAppMainLabel>().unwrap().schedule()
    }

    /// Fetches the internal default schedule
    pub fn schedule_scope<O, F: FnOnce(&mut Schedule) -> O>(&mut self, f: F) -> O {
        self.get_sub_app_mut::<SubAppMainLabel>()
            .unwrap()
            .schedule_scope(f)
    }

    /// Adds systems to a schedule on the main sub-app
    pub fn add_systems<M>(
        &mut self,
        label: impl ScheduleLabel,
        systems: impl bevy_ecs::schedule::IntoScheduleConfigs<bevy_ecs::system::ScheduleSystem, M>,
    ) -> &mut Self {
        self.get_sub_app_mut::<SubAppMainLabel>()
            .unwrap()
            .add_systems(label, systems);
        self
    }

    /// Tick the main sub-app only. Render runs on its own thread.
    pub fn tick(&mut self) {
        self.get_sub_app_mut::<SubAppMainLabel>().unwrap().tick();
    }
}
