use std::path::PathBuf;

use bevy_ecs::prelude::*;
use dare_window::{InputLog, WindowPlugin, WindowPluginConfig};

use crate::systems::{cull, open_gltf_pressed};

#[derive(Debug, Default)]
pub struct EnginePluginConfig {
    pub window: WindowPluginConfig,
    /// Loaded on the first engine tick.
    pub initial_gltf: Vec<PathBuf>,
    /// Blocks with a native file dialog before the window loop starts (same as legacy `dare` startup).
    pub prompt_gltf_on_startup: bool,
}

#[derive(Debug, Default)]
pub struct EnginePlugin {
    pub config: EnginePluginConfig,
}

impl EnginePlugin {
    pub fn new(config: EnginePluginConfig) -> Self {
        Self { config }
    }
}

impl dare_ecs::Plugin for EnginePlugin {
    fn build(&self, app: &mut dare_ecs::App) {
        app.add_plugin(WindowPlugin::new(self.config.window.clone()));

        app.world_mut().insert_resource(PendingGltfLoads {
            paths: self.config.initial_gltf.clone(),
        });

        app.schedule_scope(|schedule| {
            schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::MultiThreaded);
            schedule.add_systems(
                (open_gltf_hotkey, load_gltf_from_queue, cull)
                    .chain()
                    .in_set(dare_ecs::AppStage::Update),
            );
        });
    }
}

#[derive(Debug, Default, Resource)]
struct PendingGltfLoads {
    paths: Vec<PathBuf>,
}

fn open_gltf_hotkey(mut pending: ResMut<PendingGltfLoads>, mut input: ResMut<InputLog>) {
    let pressed_open = input
        .drain()
        .into_iter()
        .any(|event| open_gltf_pressed(&event));
    if !pressed_open {
        return;
    }
    if let Some(paths) = rfd::FileDialog::new()
        .add_filter("gltf", &["gltf", "glb"])
        .set_title("Gltf file to load")
        .pick_files()
    {
        pending.paths.extend(paths);
    }
}

fn load_gltf_from_queue(
    mut commands: Commands,
    mut pending: ResMut<PendingGltfLoads>,
    mut meshes: ResMut<dare_assets::Assets<dare_assets::Mesh>>,
    mut buffers: ResMut<dare_assets::Assets<dare_assets::Buffer>>,
) {
    if pending.paths.is_empty() {
        return;
    }
    for path in pending.paths.drain(..) {
        meshes.load_gltf(&mut commands, &mut buffers, &path);
    }
}

fn prompt_gltf_on_startup() -> Vec<PathBuf> {
    loop {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("gltf", &["gltf", "glb"])
            .set_title("Gltf file to load")
            .pick_files()
            && !paths.is_empty()
        {
            return paths;
        }
    }
}

pub fn bootstrap_engine(mut config: EnginePluginConfig) -> dare_ecs::App {
    if config.prompt_gltf_on_startup {
        config.initial_gltf.extend(prompt_gltf_on_startup());
    }

    let mut app = dare_ecs::App::new();
    app.add_plugin(EnginePlugin::new(config));
    app
}
