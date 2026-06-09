use std::future::Future;
use std::sync::OnceLock;

use bevy_ecs::prelude::*;
use smol::{Executor, Task};

fn executor() -> &'static Executor<'static> {
    static EXECUTOR: OnceLock<&'static Executor<'static>> = OnceLock::new();
    EXECUTOR.get_or_init(|| Box::leak(Box::new(Executor::new())))
}

/// Cloneable handle to the app's shared executor.
#[derive(Clone, Copy, Debug, Resource)]
pub struct SmolExecutorHandle(&'static Executor<'static>);

impl SmolExecutorHandle {
    pub fn spawn<F>(&self, fut: F) -> Task<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.0.spawn(fut)
    }

    pub fn block_on<F>(&self, fut: F) -> F::Output
    where
        F: Future,
    {
        smol::future::block_on(self.0.run(fut))
    }
}

impl std::ops::Deref for SmolExecutorHandle {
    type Target = Executor<'static>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

/// Marks the main sub-app as owning the process-wide executor.
#[derive(Clone, Copy, Debug, Resource)]
pub struct SmolExecutor(&'static Executor<'static>);

impl std::ops::Deref for SmolExecutor {
    type Target = Executor<'static>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[derive(Debug, Default)]
pub struct SmolPlugin;

impl super::Plugin for SmolPlugin {
    fn build(&self, app: &mut crate::App) {
        let ex = executor();
        let handle = SmolExecutorHandle(ex);

        app.subapps.iter_mut().for_each(|(_, subapp)| match subapp {
            super::SubAppHandle::Sync(world) => {
                world.world_mut().insert_resource(handle);
            }
            super::SubAppHandle::Async { .. } => {
                panic!("Should never encounter an async subapp during plugin startup")
            }
        });

        app.subapps
            .get_mut(&std::any::TypeId::of::<super::SubAppMainLabel>())
            .unwrap()
            .unwrap_mut_sync()
            .world_mut()
            .insert_resource(SmolExecutor(ex));
    }
}
