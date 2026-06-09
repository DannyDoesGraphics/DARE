pub mod input;
pub mod input_log;
pub mod messages;
pub mod prelude;
mod window;
mod winit_app;

use bevy_ecs::message::MessageRegistry;
use bevy_ecs::prelude::*;
use dagal::raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use dagal::winit;
pub use input::Input;
pub use input_log::*;
pub use messages::WindowMessage;
use std::sync::Arc;
pub use window::{Window, WinitWindow};
use winit_app::WinitApp;

#[derive(Debug, Clone)]
pub struct WindowHandles {
    pub raw_window_handle: Arc<RawWindowHandle>,
    pub raw_display_handle: Arc<RawDisplayHandle>,
}

unsafe impl Send for WindowHandles {}
unsafe impl Sync for WindowHandles {}

#[derive(Debug, Clone)]
pub struct WindowPluginConfig {
    pub title: String,
    pub resizable: bool,
    pub control_flow: winit::event_loop::ControlFlow,
}

impl Default for WindowPluginConfig {
    fn default() -> Self {
        Self {
            title: "DARE".into(),
            resizable: true,
            control_flow: winit::event_loop::ControlFlow::Poll,
        }
    }
}

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct WindowPlugin {
    pub config: WindowPluginConfig,
}


impl WindowPlugin {
    pub fn new(config: WindowPluginConfig) -> Self {
        Self { config }
    }
}

impl dare_ecs::Plugin for WindowPlugin {
    fn build(&self, app: &mut dare_ecs::App) {
        MessageRegistry::register_message::<WindowMessage>(app.world_mut());

        app.schedule_scope(|schedule| {
            use bevy_ecs::message::message_update_system;
            use dare_ecs::AppStage;
            schedule.add_systems(message_update_system.in_set(AppStage::First));
        });

        app.world_mut().insert_resource(InputLog::default());
        app.world_mut().init_resource::<Window>();
    }

    fn cleanup(self: Box<Self>, app: &mut dare_ecs::App) {
        let config = self.config;
        app.set_runner(Box::new(move |app: dare_ecs::App| {
            let event_loop = winit::event_loop::EventLoop::new().unwrap();
            event_loop.set_control_flow(config.control_flow);
            let mut winit = WinitApp::new(app, config);
            event_loop.run_app(&mut winit).unwrap();
        }));
    }
}
