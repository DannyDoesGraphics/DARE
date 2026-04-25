use anyhow::Result;
use tokio::sync::oneshot::error::TryRecvError;

#[derive(Debug)]
pub enum EngineCommand {
    Tick,
    LoadGltf(std::path::PathBuf),
}

#[derive(Debug, Clone)]
pub struct EngineClient {
    server_send: std::sync::mpsc::Sender<EngineCommand>,
}

impl EngineClient {
    pub fn new(server_send: std::sync::mpsc::Sender<EngineCommand>) -> Self {
        Self { server_send }
    }

    pub fn tick(&self) -> Result<()> {
        Ok(self.server_send.send(EngineCommand::Tick)?)
    }

    pub fn load_gltf(&self, path: std::path::PathBuf) -> Result<()> {
        self.server_send.send(EngineCommand::LoadGltf(path))?;
        Ok(())
    }
}

pub struct EngineServerConfig {
    pub assets_send: dare_assets::AssetManager,
    pub projections: EngineProjectionPlugins,
}

#[derive(Debug, Default)]
pub struct EngineProjectionPlugins {
    pub send_mesh_handle: Option<dare_extract::ExtractPluginSend<dare_assets::MeshHandle>>,
    pub send_transform: Option<dare_extract::ExtractPluginSend<dare_physics::Transform>>,
    pub send_bounding_box: Option<dare_extract::ExtractPluginSend<dare_physics::BoundingBox>>,
    pub send_camera: Option<dare_extract::ExtractPluginSend<crate::components::Camera>>,
}

#[derive(Debug)]
pub struct EngineServer {
    drop_sender: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl EngineServer {
    pub fn new(config: EngineServerConfig) -> Result<(Self, EngineClient)> {
        let (server_send, server_recv) = std::sync::mpsc::channel::<EngineCommand>();
        let mut app = dare_ecs::App::new();
        app.world_mut().insert_resource(config.assets_send);
        if let Some(p) = config.projections.send_mesh_handle {
            app.add_plugins(p);
        }
        if let Some(p) = config.projections.send_transform {
            app.add_plugins(p);
        }
        if let Some(p) = config.projections.send_bounding_box {
            app.add_plugins(p);
        }
        if let Some(p) = config.projections.send_camera {
            app.add_plugins(p);
        }
        app.schedule_scope(|schedule| {
            schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
        });

        let (drop_sender, mut drop_receiver) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            loop {
                match drop_receiver.try_recv() {
                    Ok(_) | Err(TryRecvError::Closed) => break,
                    Err(TryRecvError::Empty) => {}
                }
                match server_recv.recv() {
                    Err(_) => {
                        break;
                    }
                    Ok(command) => {
                        match command {
                            EngineCommand::Tick => {
                                app.tick();
                            }
                            EngineCommand::LoadGltf(path) => {
                                let asset_manager = app
                                    .world_mut()
                                    .remove_resource::<dare_assets::AssetManager>();
                                let mut commands = app.world_mut().commands();
                                if let Some(mut asset_manager) = asset_manager {
                                    asset_manager.load_gltf(&mut commands, &path);
                                    app.world_mut().insert_resource(asset_manager);
                                } else {
                                    tracing::warn!(
                                        "Asset manager does not exist, cannot load gltf scene"
                                    );
                                }
                            }
                        };
                    }
                }
            }
            drop(app);
            tracing::trace!("ENGINE SERVER STOPPED");
        });

        Ok((
            Self {
                thread: Some(thread),
                drop_sender: Some(drop_sender),
            },
            EngineClient::new(server_send),
        ))
    }
}

impl Drop for EngineServer {
    fn drop(&mut self) {
        tracing::trace!("Dropping engine manager");
        if let Some(drop_sender) = self.drop_sender.take() {
            let _ = drop_sender.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
