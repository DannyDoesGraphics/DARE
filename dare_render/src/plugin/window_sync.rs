use bevy_ecs::message::message_update_system;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use dare_ecs::AppStage;
use dare_window::{Window, WindowMessage};

use crate::RenderContext;

/// Push window changes to the render thread. On close, sync `Window::None` then drop the client.
pub fn sync_render_window(
    window: Res<Window>,
    mut messages: MessageReader<WindowMessage>,
    mut commands: Commands,
    render: Option<ResMut<RenderContext>>,
) {
    let closing = messages
        .read()
        .any(|message| matches!(message, WindowMessage::CloseRequested));

    let Some(mut render) = render else {
        return;
    };

    if render.last_sent != *window {
        if let Some(client) = render.client.as_ref() {
            if client.send_window(window.clone()).is_err() {
                render.client.take();
            } else {
                render.last_sent = window.clone();
            }
        }
    }

    if closing {
        commands.remove_resource::<RenderContext>();
    }
}

pub fn register(app: &mut dare_ecs::App) {
    app.schedule_scope(|schedule| {
        schedule.add_systems(
            sync_render_window
                .after(message_update_system)
                .in_set(AppStage::PreUpdate),
        );
    });
}
