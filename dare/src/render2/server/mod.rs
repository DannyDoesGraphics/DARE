pub mod send_types;

use crate::prelude as dare;
use crate::render2::contexts::{create_contexts, ContextsCreateInfo};
use crate::render2::physical_resource;
use crate::render2::prelude as render;
use crate::render2::server::send_types::RenderServerPacket;
use crate::util::event::EventReceiver;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use dagal::allocators::GPUAllocatorImpl;
use derivative::Derivative;

#[derive(Debug)]
pub struct RenderServer {
    thread: tokio::task::JoinHandle<()>,
}
impl RenderServer {
    pub async fn new(
        runtime: tokio::runtime::Handle,
        asset_server: dare::asset2::server::AssetServer,
        mut packet_recv: tokio::sync::mpsc::UnboundedReceiver<RenderServerPacket>,
        input_recv: EventReceiver<dare::window::input::Input>,
        ci: ContextsCreateInfo,
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
        let created_contexts = create_contexts(ci).unwrap();
        let mut world = dare::util::world::World::new();
        let thread = {
            let device_context = created_contexts.device_context;
            let graphics_context = created_contexts.graphics_context;
            let transfer_context = created_contexts.transfer_context;
            let window_context = created_contexts.window_context;
            let rt = dare::concurrent::BevyTokioRunTime::new(runtime);
            // Render thread
            tokio::task::spawn(async move {
                {
                    let mut allocator = device_context.allocator.clone();
                    world.insert_resource(
                        render::util::GPUResourceTable::<GPUAllocatorImpl>::new(
                            device_context.device.clone(),
                            &mut allocator,
                        )
                        .unwrap(),
                    );
                }
                // add senders
                world.insert_resource(input_recv);
                // add necessary resources - insert the new separate contexts
                world.insert_resource(device_context);
                world.insert_resource(graphics_context);
                world.insert_resource(transfer_context);
                world.insert_resource(window_context);
                world.insert_resource(super::frame_number::FrameCount::default());
                world.insert_resource(rt);
                world.insert_resource(asset_server.clone());
                world.insert_resource(render::components::camera::Camera::default());
                // physical resource storage
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

                let mut is_rendering = false;

                loop {
                    // close server
                    if packet_recv.is_closed() {
                        break;
                    }

                    // Always try to receive packets without blocking
                    while let Ok(packet) = packet_recv.try_recv() {
                        match packet.request {
                            render::RenderServerRequest::RenderStart => {
                                is_rendering = true;
                            }
                            render::RenderServerRequest::RenderEnd => {
                                is_rendering = false;
                            }
                            render::RenderServerRequest::Stop => {
                                let mut shutdown_schedule = becs::Schedule::default();
                                shutdown_schedule.add_systems(
                                    render::systems::shutdown_system::render_server_shutdown_system,
                                );
                                shutdown_schedule.run(&mut world);
                                return; // Exit the loop and function
                            }
                            render::RenderServerRequest::SurfaceUpdate {
                                dimensions: _,
                                raw_handles: _,
                            } => {
                                // Implement surface update with separate contexts
                                // Use a system-style approach to handle the borrow checker properly
                                let mut update_schedule = becs::Schedule::default();
                                update_schedule.add_systems(|
                                    device_context: becs::Res<'_, super::contexts::DeviceContext>,
                                    mut window_context: becs::ResMut<'_, super::contexts::WindowContext>,
                                | {
                                    let window_handles = window_context.window_handles.clone();
                                    match window_context.update_surface(
                                        super::contexts::SurfaceContextUpdateInfo {
                                            instance: &device_context.instance,
                                            physical_device: &device_context.physical_device,
                                            allocator: device_context.allocator.clone(),
                                            raw_handles: window_handles,
                                            frames_in_flight: None, // Use default
                                        }
                                    ) {
                                        Ok(()) => {}
                                        Err(e) => {
                                            tracing::error!("Failed to update surface: {}", e);
                                        }
                                    }
                                });
                                update_schedule.run(&mut world);
                            }
                        };
                        packet.callback.map(|v| v.send(()));
                    }

                    // If we're in rendering mode, run a frame
                    if is_rendering {
                        schedule.run(&mut world);
                    }

                    // Small yield to prevent blocking the async runtime completely
                    tokio::task::yield_now().await;
                }
                tracing::trace!("Stopping render manager");
                // Manually extract contexts in dependency order to ensure proper Vulkan cleanup
                // Graphics and Transfer contexts depend on Device, so drop them first
                let _graphics_context = world.remove_resource::<super::contexts::GraphicsContext>();
                let _transfer_context = world.remove_resource::<super::contexts::TransferContext>();
                let _window_context = world.remove_resource::<super::contexts::WindowContext>();
                // Device context contains the core Vulkan objects and should be dropped last
                let _device_context = world.remove_resource::<super::contexts::DeviceContext>();
                // Now drop the world with remaining resources
                drop(world);
                // Contexts will drop in reverse order of declaration (device_context last)
                tracing::trace!("RENDER SERVER STOPPED");
            })
        };
        // Note: Render thread management is now simplified without RenderContext
        Self {
            thread,
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

    /// Starts continuous rendering
    pub fn start_rendering(&self) -> Result<()> {
        self.send(render::RenderServerRequest::RenderStart)
    }

    /// Stops continuous rendering
    pub fn stop_rendering(&self) -> Result<()> {
        self.send(render::RenderServerRequest::RenderEnd)
    }

    /// Starts continuous rendering with blocking for callback
    pub fn start_rendering_blocking(&self) -> Result<()> {
        self.send_blocking(render::RenderServerRequest::RenderStart)
    }

    /// Stops continuous rendering with blocking for callback
    pub fn stop_rendering_blocking(&self) -> Result<()> {
        self.send_blocking(render::RenderServerRequest::RenderEnd)
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
