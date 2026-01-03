use anyhow::Result;
use bevy_ecs::prelude as becs;
use tokio::sync::oneshot::error::TryRecvError;

#[derive(Debug)]
enum EnginePacket {
    Tick,
    LoadGltf(std::path::PathBuf),
}

#[derive(Debug, Clone)]
pub struct EngineClient {
    server_send: std::sync::mpsc::Sender<EnginePacket>,
}

impl EngineClient {
    pub fn new(server_send: std::sync::mpsc::Sender<EnginePacket>) -> Self {
        Self { server_send }
    }

    pub fn tick(&self) -> Result<()> {
        Ok(self.server_send.send(EnginePacket::Tick)?)
    }

    pub fn load_gltf(&self, path: std::path::PathBuf) -> Result<()> {
        self.server_send.send(EnginePacket::LoadGltf(path))?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct EngineServer {
    drop_sender: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl EngineServer {
    pub fn new<F>(init: F) -> Result<(Self, EngineClient)>
    where
        F: FnOnce(&mut becs::World, &mut becs::Schedule) + Send + 'static,
    {
        let (server_send, server_recv) = std::sync::mpsc::channel::<EnginePacket>();
        let mut world = becs::World::new();
        let assets = dare_assets::AssetManager::new(16);
        world.insert_resource(assets);
        let mut scheduler = becs::Schedule::default();
        scheduler.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);

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
                    Ok(packet) => {
                        match packet {
                            EnginePacket::Tick => {
                                scheduler.run(&mut world);
                            }
                            EnginePacket::LoadGltf(path) => {
                                let asset_manager =
                                    world.remove_resource::<dare_assets::AssetManager>();
                                let mut commands = world.commands();
                                if let Some(mut asset_manager) = asset_manager {
                                    asset_manager.load_gltf(&mut commands, &path);
                                    world.insert_resource(asset_manager);
                                } else {
                                    tracing::warn!("Asset manager does not exist, cannot load gltf scene");
                                }
                            }
                        };
                    }
                }
            }
            drop(world);
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
