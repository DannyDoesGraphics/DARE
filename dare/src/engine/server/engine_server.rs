use crate::prelude as dare;
use crate::util::entity_linker::ComponentsLinkerSender;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use bevy_ecs::prelude::IntoSystemConfigs;

#[derive(Debug, Clone)]
pub struct EngineClient {
    server_send: tokio::sync::mpsc::UnboundedSender<()>,
}

impl EngineClient {
    pub fn new(server_send: tokio::sync::mpsc::UnboundedSender<()>) -> Self {
        Self { server_send }
    }

    pub fn tick(&self) -> Result<()> {
        Ok(self.server_send.send(())?)
    }
}

#[derive(Debug)]
pub struct EngineServer {
    thread: tokio::task::JoinHandle<()>,
}

impl EngineServer {
    pub fn new(
        runtime: tokio::runtime::Handle,
        mut server_recv: tokio::sync::mpsc::UnboundedReceiver<()>,
        asset_server: dare::asset2::server::AssetServer,
        surface_link_send: &ComponentsLinkerSender<dare::engine::components::Surface>,
        texture_link_send: &ComponentsLinkerSender<dare::engine::components::Material>,
        transform_link_send: &ComponentsLinkerSender<dare::physics::components::Transform>,
        bb_link_send: &ComponentsLinkerSender<dare::render::components::BoundingBox>,
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
        init_schedule.run(&mut world);

        let mut scheduler = becs::Schedule::default();
        surface_link_send.attach_to_world(&mut scheduler);
        transform_link_send.attach_to_world(&mut scheduler);
        bb_link_send.attach_to_world(&mut scheduler);
        texture_link_send.attach_to_world(&mut scheduler);

        let thread = rt.runtime.spawn(async move {
            loop {
                if server_recv.is_closed() {
                    break;
                }
                match server_recv.recv().await {
                    None => {}
                    Some(_) => {
                        scheduler.run(&mut world);
                    }
                }
            }
            drop(world);
            tracing::trace!("ENGINE SERVER STOPPED");
        });

        Ok(Self { thread })
    }
}

impl Drop for EngineServer {
    fn drop(&mut self) {
        tracing::trace!("Dropping engine manager");
        //self.thread.abort();
    }
}
