//! Some main issues with the previous renderer is how horrendous the threading model is.
//! This resolves it.

use bevy_ecs::prelude::*;
use dagal::ash::vk;
use dare_window::WindowHandles;

pub mod components;
mod contexts;
pub mod extract;
mod frame;
mod resource_manager;
pub mod snapshot;
mod systems;
mod timer;
mod transfer_belt;

/// Configuration for creating a RenderServer.
#[derive(Debug)]
pub struct RenderServerConfig {
    pub extent: vk::Extent2D,
    pub window_handles: WindowHandles,
    pub asset_manager: dare_assets::AssetManager,
    pub frames_in_flight: usize,
    pub transfer_buffer_size: u64,
    pub max_transfers: u64,
    pub projections: RenderProjectionPlugins,
}

#[derive(Debug, Default)]
pub struct RenderProjectionPlugins {
    pub recv_mesh_handle: Option<dare_extract::ExtractPluginRecv<dare_assets::MeshHandle>>,
    pub recv_transform: Option<dare_extract::ExtractPluginRecv<dare_physics::Transform>>,
    pub recv_bounding_box: Option<dare_extract::ExtractPluginRecv<dare_physics::BoundingBox>>,
    pub recv_camera: Option<dare_extract::ExtractPluginRecv<dare_engine::Camera>>,
}

/// Handle to render server thread.
///
/// Runs a separate thread with its own Tokio runtime and Bevy ECS world.
#[derive(Debug)]
pub struct RenderServer {
    thread: Option<std::thread::JoinHandle<()>>,
    drop_sender: Option<tokio::sync::oneshot::Sender<()>>,
}

/// Client to communicate with render server.
#[derive(Debug, Clone)]
pub struct RenderClient {
    command_sender: std::sync::mpsc::Sender<RenderServerCommand>,
}

impl RenderClient {
    pub fn new(command_sender: std::sync::mpsc::Sender<RenderServerCommand>) -> Self {
        Self { command_sender }
    }

    pub fn resize(&self, extent: vk::Extent2D) -> anyhow::Result<()> {
        Ok(self
            .command_sender
            .send(RenderServerCommand::Resize(extent))?)
    }

    pub fn recreate(&self, size: vk::Extent2D, handles: WindowHandles) -> anyhow::Result<()> {
        Ok(self
            .command_sender
            .send(RenderServerCommand::Recreate { size, handles })?)
    }

    pub fn set_render(
        &self,
        handle: dare_assets::MeshHandle,
        should_render: bool,
    ) -> anyhow::Result<()> {
        Ok(self.command_sender.send(RenderServerCommand::SetRender {
            handle,
            should_render,
        })?)
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        Ok(self.command_sender.send(RenderServerCommand::Stop)?)
    }
}

pub enum RenderServerCommand {
    Resize(vk::Extent2D),
    Recreate {
        size: vk::Extent2D,
        handles: WindowHandles,
    },
    SetRender {
        handle: dare_assets::MeshHandle,
        should_render: bool,
    },
    Stop,
}

impl RenderServer {
    pub fn new(config: RenderServerConfig) -> (Self, RenderClient) {
        let (drop_sender, mut drop_receiver) = tokio::sync::oneshot::channel();
        let (command_sender, command_receiver) = std::sync::mpsc::channel::<RenderServerCommand>();
        let window_handles = config.window_handles;
        let extent = config.extent;
        let projections = config.projections;
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let _guard = runtime.enter();
            let mut app = dare_ecs::App::new();
            app.world_mut().insert_resource(timer::Timer {
                last_recorded: None,
            });
            if let Some(p) = projections.recv_mesh_handle {
                app.add_plugins(p);
            }
            if let Some(p) = projections.recv_transform {
                app.add_plugins(p);
            }
            if let Some(p) = projections.recv_bounding_box {
                app.add_plugins(p);
            }
            if let Some(p) = projections.recv_camera {
                app.add_plugins(p);
            }

            // Core context
            let (core_context, surface): (contexts::CoreContext, dagal::wsi::SurfaceQueried) =
                contexts::CoreContext::new(&window_handles).unwrap();
            let swapchain_context: contexts::SwapchainContext<dagal::allocators::GPUAllocatorImpl> =
                contexts::SwapchainContext::new(surface, extent, &core_context).unwrap();
            let present_context: contexts::PresentContext =
                contexts::PresentContext::new(&core_context, config.frames_in_flight).unwrap();

            // Transfer belt
            let transfer_manager: transfer_belt::TransferManager<
                dagal::allocators::GPUAllocatorImpl,
            > = transfer_belt::TransferManager::new(
                core_context.device.clone(),
                core_context
                    .queue_allocator
                    .retrieve_queues(None, vk::QueueFlags::TRANSFER, Some(1))
                    .unwrap()
                    .pop()
                    .unwrap(),
                core_context.allocator.clone(),
                config.transfer_buffer_size,
                config.max_transfers,
            )
            .unwrap();

            app.add_plugins(resource_manager::ResourceManagerPlugin::new(
                config.asset_manager,
                config.frames_in_flight as u16,
            ));
            app.world_mut().insert_resource(core_context);
            app.world_mut().insert_resource(swapchain_context);
            app.world_mut().insert_resource(present_context);
            app.world_mut().insert_non_send_resource(transfer_manager);

            app.schedule_scope(|schedule| {
                schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
                schedule.add_systems(systems::render_system::<dagal::allocators::GPUAllocatorImpl>);
            });

            loop {
                match drop_receiver.try_recv() {
                    Ok(_) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => break,
                    Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
                }
                let mut stop = false;
                while let Ok(command) = command_receiver.try_recv() {
                    match command {
                        RenderServerCommand::Resize(extent) => {
                            app.world_mut().resource_scope(
                                |world,
                                 mut swapchain_context: Mut<
                                    contexts::SwapchainContext<dagal::allocators::GPUAllocatorImpl>,
                                >| {
                                    let present_context =
                                        world.get_resource::<contexts::PresentContext>().unwrap();
                                    let core_context =
                                        world.get_resource::<contexts::CoreContext>().unwrap();
                                    if let Err(err) = swapchain_context.resize(
                                        extent,
                                        present_context,
                                        core_context,
                                    ) {
                                        tracing::error!(?err, ?extent, "Swapchain resize failed");
                                    }
                                },
                            );
                        }
                        RenderServerCommand::Recreate { size, handles } => {
                            app.world_mut().resource_scope(
                                |world,
                                 mut swapchain_context: Mut<
                                    contexts::SwapchainContext<dagal::allocators::GPUAllocatorImpl>,
                                >| {
                                    let present_context =
                                        world.get_resource::<contexts::PresentContext>().unwrap();
                                    let core_context =
                                        world.get_resource::<contexts::CoreContext>().unwrap();
                                    if let Err(err) = swapchain_context.recreate(
                                        size,
                                        handles.clone(),
                                        present_context,
                                        core_context,
                                    ) {
                                        tracing::error!(?err, ?size, "Swapchain recreate failed");
                                    }
                                },
                            );
                        }
                        RenderServerCommand::SetRender {
                            handle: _,
                            should_render: _,
                        } => {
                            tracing::warn!("Tried to set state of unimplemented type");
                        }
                        RenderServerCommand::Stop => {
                            stop = true;
                            break;
                        }
                    }
                }
                if stop {
                    break;
                }
                app.tick();
            }

            // shut down
            if let Some(core_context) = app.world().get_resource::<contexts::CoreContext>() {
                let _ = unsafe { core_context.device.get_handle().device_wait_idle() };
            }
            // drop all contexts here
            let present_context = app
                .world_mut()
                .remove_resource::<contexts::PresentContext>();
            let swapchain_context = app
                .world_mut()
                .remove_resource::<contexts::SwapchainContext<dagal::allocators::GPUAllocatorImpl>>(
                );
            let core_context = app.world_mut().remove_resource::<contexts::CoreContext>();
            drop(app);
            drop(present_context);
            drop(swapchain_context);
            drop(core_context);
        });
        (
            Self {
                thread: Some(thread),
                drop_sender: Some(drop_sender),
            },
            RenderClient::new(command_sender),
        )
    }
}

impl Drop for RenderServer {
    fn drop(&mut self) {
        if let Some(drop_sender) = self.drop_sender.take() {
            let _ = drop_sender.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
