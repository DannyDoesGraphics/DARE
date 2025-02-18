use crate::prelude as dare;
use crate::render2::server::IrSend;
use crate::util::entity_linker::ComponentsLinkerSender;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use bevy_ecs::prelude::IntoSystemConfigs;

#[derive(Debug)]
pub struct EngineServer {
    sender: tokio::sync::mpsc::Sender<()>,
    thread: tokio::task::JoinHandle<()>,
}
unsafe impl Send for EngineServer {}
unsafe impl Sync for EngineServer {}

impl EngineServer {
    pub fn new(
        asset_server: dare::asset2::server::AssetServer,
        send: IrSend,
        surface_link_send: &ComponentsLinkerSender<dare::engine::components::Surface>,
        texture_link_send: &ComponentsLinkerSender<dare::engine::components::Material>,
        transform_link_send: &ComponentsLinkerSender<dare::physics::components::Transform>,
        bb_link_send: &ComponentsLinkerSender<dare::render::components::BoundingBox>,
    ) -> Result<Self> {
        let rt = dare::concurrent::BevyTokioRunTime::default();

        let mut world = becs::World::new();
        world.insert_resource(rt.clone());
        world.insert_resource(asset_server);
        world.insert_resource(send);

        let mut init_schedule = becs::Schedule::default();
        init_schedule.add_systems(super::super::init_assets::init_assets);
        surface_link_send.attach_to_world(&mut init_schedule);
        transform_link_send.attach_to_world(&mut init_schedule);
        bb_link_send.attach_to_world(&mut init_schedule);
        texture_link_send.attach_to_world(&mut init_schedule);
        init_schedule.run(&mut world);

        let mut scheduler = becs::Schedule::default();
        surface_link_send.attach_to_world(&mut scheduler);
        transform_link_send.attach_to_world(&mut scheduler);
        bb_link_send.attach_to_world(&mut scheduler);
        texture_link_send.attach_to_world(&mut scheduler);

        let (send, mut recv) = tokio::sync::mpsc::channel::<()>(32);
        let thread = rt.runtime.spawn_blocking(move || {
            loop {
                match recv.try_recv() {
                    Ok(_) => {
                        scheduler.run(&mut world);
                    }
                    Err(e) => match e {
                        tokio::sync::mpsc::error::TryRecvError::Empty => {}
                        tokio::sync::mpsc::error::TryRecvError::Disconnected => break,
                    },
                }
            }
            drop(world);
            tracing::trace!("ENGINE SERVER STOPPED");
        });

        Ok(Self {
            sender: send,
            thread,
        })
    }

    /// stops the engine manager
    pub fn stop(&self) {
        self.thread.abort();
    }

    pub async fn tick(&self) -> Result<()> {
        Ok(self.sender.send(()).await?)
    }
}

impl Drop for EngineServer {
    fn drop(&mut self) {
        tracing::trace!("Dropping engine manager");
        self.thread.abort();
    }
}
