use bevy_ecs::prelude::*;
use dagal::winit;
use dare_ecs::{App, AppStage, ExtractPlugin, Plugin, SubAppMainLabel};
use dare_window::{Input, InputLog};

use crate::plugin::RenderSubAppLabel;

#[derive(Resource, Debug, PartialEq, Eq, Copy, Clone, Default)]
pub enum RenderMode {
    #[default]
    Rasterize,
    PathTrace,
}

impl RenderMode {
    pub fn is_raster(&self) -> bool {
        matches!(self, RenderMode::Rasterize)
    }

    pub fn toggled(self) -> Self {
        match self {
            RenderMode::Rasterize => RenderMode::PathTrace,
            RenderMode::PathTrace => RenderMode::Rasterize,
        }
    }
}

/// Owns the [`RenderMode`] state and sets up `tab` control behavior
#[derive(Default)]
pub struct RenderModePlugin;

impl Plugin for RenderModePlugin {
    fn build(&self, app: &mut App) {
        // Source of truth in the main world.
        app.world_mut().init_resource::<RenderMode>();

        app.get_sub_app_mut::<RenderSubAppLabel>()
            .expect("RenderModePlugin requires the render sub-app")
            .world_mut()
            .init_resource::<RenderMode>();

        app.schedule_scope(|schedule| {
            schedule.add_systems(toggle_render_mode.in_set(AppStage::Update));
        });

        app.add_plugin(ExtractPlugin::<
            RenderMode,
            RenderSubAppLabel,
            SubAppMainLabel,
        >::new(
            |world| world.get_resource::<RenderMode>().copied(),
            |world, modes| {
                // `last` wins if several sim ticks batched between render ticks.
                let Some(&mode) = modes.last() else {
                    return;
                };
                let changed = world.get_resource::<RenderMode>().copied() != Some(mode);
                world.insert_resource(mode);
                if changed {
                    tracing::info!("Render mode set to: {:?}", mode);
                }
            },
        ));
    }
}

fn toggle_render_mode(input: Res<InputLog>, mut mode: ResMut<RenderMode>) {
    if !input.events().iter().any(toggle_pressed) {
        return;
    }
    *mode = mode.toggled();
}

fn toggle_pressed(input: &Input) -> bool {
    let Input::KeyEvent { event, .. } = input else {
        return false;
    };
    event.state.is_pressed()
        && !event.repeat
        && event.physical_key == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Tab)
}
