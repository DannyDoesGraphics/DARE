pub mod send_types;

use crate::prelude as dare;
use crate::render2::physical_resource;
use crate::render2::prelude as render;
use crate::render2::render_assets::storage::RenderAssetManagerStorage;
use crate::render2::render_context::{RenderContext, RenderContextCreateInfo};
use crate::render2::server::send_types::RenderServerPacket;
use crate::util::event::EventReceiver;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::winit;
use derivative::Derivative;
use std::any::Any;
use std::cmp::PartialEq;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug)]
pub struct RenderServer {
    thread: tokio::task::JoinHandle<()>,
    render_context: RenderContext,
}
impl RenderServer {
    pub async fn new(
        runtime: tokio::runtime::Handle,
        asset_server: dare::asset2::server::AssetServer,
        mut packet_recv: tokio::sync::mpsc::UnboundedReceiver<RenderServerPacket>,
        input_recv: EventReceiver<dare::window::input::Input>,
        ci: RenderContextCreateInfo,
        surface_link_recv: dare::util::entity_linker::ComponentsLinkerReceiver<
            dare::engine::components::Surface,
        >,
        texture_link_recv: dare::util::entity_linker::ComponentsLinkerReceiver<
            dare::engine::components::Material,
        >,
        transform_link_recv: dare::util::entity_linker::ComponentsLinkerReceiver<
            dare::physics::components::Transform,
        >,
        bb_link_recv: dare::util::entity_linker::ComponentsLinkerReceiver<
            render::components::BoundingBox,
        >,
        name_link_recv: dare::util::entity_linker::ComponentsLinkerReceiver<
            dare::engine::components::Name,
        >,
    ) -> Self {
        println!("Starting");
        //let (new_send, mut new_recv) = crossbeam_channel::unbounded::<RenderServerPacket>();
        let render_context = RenderContext::new(ci).unwrap();
        let mut world = dare::util::world::World::new();
        let thread = {
            let render_context = render_context.clone();
            let rt = dare::concurrent::BevyTokioRunTime::new(runtime);
            // Render thread
            tokio::task::spawn(async move {
                {
                    let mut allocator = render_context.inner.allocator.clone();
                    world.insert_resource(
                        render::util::GPUResourceTable::<GPUAllocatorImpl>::new(
                            render_context.inner.device.clone(),
                            &mut allocator,
                        )
                        .unwrap(),
                    );
                }
                // add senders
                world.insert_resource(input_recv);
                // add necessary resources
                world.insert_resource(render_context.clone());
                world.insert_resource(super::frame_number::FrameCount::default());
                world.insert_resource(rt);
                world.insert_resource(asset_server.clone());
                world.insert_resource(render::components::camera::Camera::default());
                // physical resource storage
                world.insert_resource(RenderAssetManagerStorage::<
                    physical_resource::RenderBuffer<GPUAllocatorImpl>,
                >::new(asset_server.clone()));
                world.insert_resource(RenderAssetManagerStorage::<
                    physical_resource::RenderImage<GPUAllocatorImpl>,
                >::new(asset_server.clone()));
                world.insert_resource(physical_resource::PhysicalResourceStorage::<
                    dare::asset2::assets::SamplerAsset,
                >::new(asset_server.clone()));
                world.insert_resource(super::systems::delta_time::DeltaTime::default());
                let mut schedule = becs::Schedule::default();
                // links
                surface_link_recv.attach_to_world(&mut world, &mut schedule);
                texture_link_recv.attach_to_world(&mut world, &mut schedule);
                transform_link_recv.attach_to_world(&mut world, &mut schedule);
                bb_link_recv.attach_to_world(&mut world, &mut schedule);
                name_link_recv.attach_to_world(&mut world, &mut schedule);
                // physical resources
                world.insert_resource(physical_resource::PhysicalResourceStorage::<
                    physical_resource::RenderBuffer<GPUAllocatorImpl>,
                >::new(asset_server.clone()));
                world.insert_resource(physical_resource::PhysicalResourceStorage::<
                    physical_resource::RenderImage<GPUAllocatorImpl>,
                >::new(asset_server.clone()));
                // misc
                schedule.add_systems(super::systems::delta_time::delta_time_update);
                schedule.add_systems(super::components::camera::camera_system);
                // rendering
                schedule.add_systems(super::present_system::present_system_begin);
                loop {
                    // close server
                    if packet_recv.is_closed() {
                        break;
                    }
                    match packet_recv.recv().await {
                        Some(packet) => {
                            match packet.request {
                                render::RenderServerRequest::Render => {
                                    schedule.run(&mut world);
                                }
                                render::RenderServerRequest::Stop => {
                                    let mut shutdown_schedule = becs::Schedule::default();
                                    shutdown_schedule.add_systems(render::systems::shutdown_system::render_server_shutdown_system);
                                    shutdown_schedule.run(&mut world);
                                    break;
                                }
                                render::RenderServerRequest::SurfaceUpdate {
                                    dimensions,
                                    raw_handles,
                                } => {
                                    if let Err(e) = render_context.update_surface() {
                                        eprintln!("Failed to update surface: {}", e);
                                    }
                                }
                            };
                            packet.callback.map(|v| v.send(()));
                        }
                        None => {}
                    }
                }
                tracing::trace!("Stopping render manager");
                // drop world
                drop(world);
                tracing::trace!("RENDER SERVER STOPPED");
            })
        };
        render_context
            .bind_render_thread(thread.abort_handle())
            .await;
        Self {
            thread,
            render_context,
        }
    }
}

impl Drop for RenderServer {
    fn drop(&mut self) {}
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct RenderClient {
    sender: tokio::sync::mpsc::UnboundedSender<RenderServerPacket>,
    input_sender: dare::util::event::EventSender<dare::window::input::Input>,
}

impl RenderClient {
    pub fn new(
        server_send: tokio::sync::mpsc::UnboundedSender<RenderServerPacket>,
        input_sender: dare::util::event::EventSender<dare::window::input::Input>,
    ) -> Self {
        Self {
            sender: server_send,
            input_sender,
        }
    }

    pub fn input_send(&self) -> &dare::util::event::EventSender<dare::window::input::Input> {
        &self.input_sender
    }

    /// Sends with blocking for a callback
    pub fn send_blocking(&self, request: render::RenderServerRequest) -> Result<()> {
        let (send, recv) = tokio::sync::oneshot::channel::<()>();
        self.sender.send(RenderServerPacket {
            callback: Some(send),
            request,
        })?;
        recv.blocking_recv()?;
        Ok(())
    }

    /// Sends without awaiting on a callback
    pub fn send(&self, request: render::RenderServerRequest) -> Result<()> {
        self.sender.send(RenderServerPacket {
            callback: None,
            request,
        })?;
        Ok(())
    }
}
