mod window_sync;

use bevy_ecs::prelude::*;
use dare_ecs::{App, AppStage, Plugin, SubApp};
use dare_window::Window;

pub use crate::{RenderClient, RenderServerSpawnConfig};

pub struct RenderSubAppLabel;
impl dare_ecs::SubAppLabel for RenderSubAppLabel {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RenderPluginConfig {
    pub frames_in_flight: usize,
    pub transfer_buffer_size: u64,
    pub max_transfers: u64,
    pub ttl: u16,
}

impl Default for RenderPluginConfig {
    fn default() -> Self {
        Self {
            frames_in_flight: 3,
            transfer_buffer_size: 1024 * 1024 * 64,
            max_transfers: 16,
            ttl: 128,
        }
    }
}

#[derive(Resource)]
pub struct RenderContext {
    pub client: Option<RenderClient>,
    pub last_sent: Window,
}

pub struct RenderPlugin {
    pub config: RenderPluginConfig,
}

impl RenderPlugin {
    pub fn new(config: RenderPluginConfig) -> Self {
        Self { config }
    }
}

fn frame_render_start(render: Option<ResMut<RenderContext>>) {
    let Some(mut render) = render else {
        return;
    };
    let Some(client) = render.client.as_ref() else {
        return;
    };
    if client.frame_render_start().is_err() {
        render.client.take();
    }
}

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        if app.get_sub_app::<RenderSubAppLabel>().is_none() {
            app.insert_sub_app::<RenderSubAppLabel>(SubApp::new());
        }

        // buffers
        app.add_plugin(dare_assets::AssetSync::<
            dare_assets::Mesh,
            dare_ecs::SubAppMainLabel,
            RenderSubAppLabel,
        >::new(self.config.ttl));
        app.add_plugin(dare_assets::AssetSync::<
            dare_assets::Buffer,
            dare_ecs::SubAppMainLabel,
            RenderSubAppLabel,
        >::new(self.config.ttl));

        app.get_sub_app_mut::<RenderSubAppLabel>()
            .unwrap()
            .world_mut()
            .init_resource::<Window>();

        window_sync::register(app);
        app.schedule_scope(|schedule| {
            schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
            schedule.add_systems(frame_render_start.in_set(AppStage::Update));
        });
    }

    fn cleanup(self: Box<Self>, app: &mut App) {
        let render_sub_app = app
            .remove_sub_app::<RenderSubAppLabel>()
            .expect("RenderPlugin cleanup: RenderSubAppLabel missing");

        let client = RenderClient::spawn(RenderServerSpawnConfig {
            frames_in_flight: self.config.frames_in_flight,
            transfer_buffer_size: self.config.transfer_buffer_size,
            max_transfers: self.config.max_transfers,
            render_sub_app,
        });

        app.world_mut().insert_resource(RenderContext {
            client: Some(client),
            last_sent: Window::None,
        });
    }
}
