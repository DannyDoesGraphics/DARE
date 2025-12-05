use crate::prelude as dare;
use crate::util::entity_linker::ComponentsLinkerSender;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use tokio::sync::oneshot::error::TryRecvError;

#[derive(Debug, Clone)]
pub struct EngineClient {
    server_send: std::sync::mpsc::Sender<()>,
}

impl EngineClient {
    pub fn new(server_send: std::sync::mpsc::Sender<()>) -> Self {
        Self { server_send }
    }

    pub fn tick(&self) -> Result<()> {
        Ok(self.server_send.send(())?)
    }
}

#[derive(Debug)]
pub struct EngineServer {
    drop_sender: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl EngineServer {
    pub fn new(
        server_recv: std::sync::mpsc::Receiver<()>,
    ) -> Result<Self> {
        let mut world = becs::World::new();
        world.insert_resource(crate::asset_system::AssetManager::new());

        let mut init_schedule = becs::Schedule::default();
        init_schedule.add_systems(super::super::init_assets::init_assets);
        init_schedule.run(&mut world);

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
                    Ok(_) => {
                        scheduler.run(&mut world);
                    }
                }
            }
            drop(world);
            tracing::trace!("ENGINE SERVER STOPPED");
        });

        Ok(Self {
            thread: Some(thread),
            drop_sender: Some(drop_sender),
        })
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
