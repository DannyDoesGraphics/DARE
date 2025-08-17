use std::sync::mpsc::RecvError;

use crate::prelude as dare;
use crate::util::entity_linker::ComponentsLinkerSender;
use anyhow::Result;
use bevy_ecs::prelude as becs;

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
    drop_signal: tokio_util::sync::CancellationToken,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl EngineServer {
    pub fn new(
        runtime: tokio::runtime::Handle,
        server_recv: std::sync::mpsc::Receiver<()>,
        asset_server: dare::asset2::server::AssetServer,
        surface_link_send: &ComponentsLinkerSender<dare::engine::components::Surface>,
        texture_link_send: &ComponentsLinkerSender<dare::engine::components::Material>,
        transform_link_send: &ComponentsLinkerSender<dare::physics::components::Transform>,
        bb_link_send: &ComponentsLinkerSender<dare::render::components::BoundingBox>,
        name_link_send: &ComponentsLinkerSender<dare::engine::components::Name>,
    ) -> Result<Self> {
        let rt = dare::concurrent::BevyTokioRunTime::new(runtime);

        let mut world = becs::World::new();
        world.insert_resource(rt.clone());
        world.insert_resource(asset_server);

        let mut init_schedule = becs::Schedule::default();
        init_schedule.add_systems(super::super::init_assets::init_assets);
        surface_link_send.attach_to_world(&mut init_schedule);
        transform_link_send.attach_to_world(&mut init_schedule);
        bb_link_send.attach_to_world(&mut init_schedule);
        texture_link_send.attach_to_world(&mut init_schedule);
        name_link_send.attach_to_world(&mut init_schedule);
        init_schedule.run(&mut world);

        let mut scheduler = becs::Schedule::default();
        surface_link_send.attach_to_world(&mut scheduler);
        transform_link_send.attach_to_world(&mut scheduler);
        bb_link_send.attach_to_world(&mut scheduler);
        texture_link_send.attach_to_world(&mut scheduler);
        name_link_send.attach_to_world(&mut scheduler);

        let cancellation = tokio_util::sync::CancellationToken::new();
        let cancel = cancellation.clone();
        let thread = std::thread::spawn(move || {
            loop {
                if cancel.is_cancelled() {
                    break;
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

        Ok(Self { thread: Some(thread), drop_signal: cancellation })
    }
}

impl Drop for EngineServer {
    fn drop(&mut self) {
        tracing::trace!("Dropping engine manager");
        self.drop_signal.cancel();
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
