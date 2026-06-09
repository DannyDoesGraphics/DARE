use bevy_ecs::message::message_update_system;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use dare_ecs::AppStage;
use dare_window::{Window, WindowMessage};

use crate::RenderContext;

pub fn handle_window_messages(mut messages: MessageReader<WindowMessage>, mut commands: Commands) {
    for message in messages.read() {
        match message {
            WindowMessage::CloseRequested => {
                commands.remove_resource::<RenderContext>();
            }
            WindowMessage::Resized { .. } | WindowMessage::Suspended => {}
        }
    }
}

pub fn sync_render_window(window: Res<Window>, render: Option<ResMut<RenderContext>>) {
    let Some(mut render) = render else {
        return;
    };
    if render.last_sent == *window {
        return;
    }
    let Some(client) = render.client.as_ref() else {
        return;
    };
    if client.send_window(window.clone()).is_err() {
        render.client.take();
        return;
    }
    render.last_sent = window.clone();
}

pub fn register(app: &mut dare_ecs::App) {
    app.schedule_scope(|schedule| {
        schedule.add_systems(
            handle_window_messages
                .after(message_update_system)
                .in_set(AppStage::First),
        );
        schedule.add_systems(sync_render_window.in_set(AppStage::PreUpdate));
    });
}
