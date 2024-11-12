use bevy_ecs::prelude as becs;

#[derive(Clone, becs::Resource)]
pub struct BevyTokioRunTime {
    pub runtime: tokio::runtime::Handle,
}

impl Default for BevyTokioRunTime {
    fn default() -> Self {
        Self {
            runtime: tokio::runtime::Handle::current(),
        }
    }
}
