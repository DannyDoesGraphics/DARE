//! Some main issues with the previous renderer is how horrendous the threading model is.
//! This resolves it.

use bevy_ecs::world::Mut;
use dagal::{allocators::Allocator, ash::vk};
use tokio::sync::oneshot::error::TryRecvError;

use crate::window::WindowHandles;

mod contexts;
mod frame;
mod systems;
mod timer;

#[derive(Debug)]
pub struct RenderServer {
    thread: Option<std::thread::JoinHandle<()>>,
    drop_sender: Option<tokio::sync::oneshot::Sender<()>>,
    pub packet_sender: crossbeam_channel::Sender<RenderServerPacket>,
}

pub enum RenderServerPacket {
    Resize(vk::Extent2D),
    Recreate {
        size: vk::Extent2D,
        handles: WindowHandles,
    },
}

impl RenderServer {
    pub fn new<A: Allocator>(extent: vk::Extent2D, window_handles: WindowHandles) -> Self {
        let (drop_sender, mut drop_receiver) = tokio::sync::oneshot::channel();
        let (packet_sender, packet_receiver) = crossbeam_channel::unbounded::<RenderServerPacket>();
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let _guard = runtime.enter();
            let mut world: bevy_ecs::world::World = bevy_ecs::world::World::new();
            world.insert_resource(timer::Timer {
                last_recorded: None,
            });

            // Core context
            let (core_context, surface): (contexts::CoreContext, dagal::wsi::SurfaceQueried) =
                contexts::CoreContext::new(&window_handles).unwrap();
            let swapchain_context: contexts::SwapchainContext<A> =
                contexts::SwapchainContext::<A>::new(surface, extent, &core_context).unwrap();
            let present_context: contexts::PresentContext =
                contexts::PresentContext::new(&core_context, 3).unwrap(); // 3 frames in flight
            world.insert_resource(core_context);
            world.insert_resource(swapchain_context);
            world.insert_resource(present_context);

            let mut schedule = bevy_ecs::schedule::Schedule::default();
            schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
            schedule.add_systems(systems::render_system::<A>);

            loop {
                match drop_receiver.try_recv() {
                    Ok(_) | Err(TryRecvError::Closed) => break,
                    Err(TryRecvError::Empty) => {}
                }
                while let Ok(packet) = packet_receiver.try_recv() {
                    match packet {
                        RenderServerPacket::Resize(extent) => {
                            world.resource_scope(
                                |world, mut swapchain_context: Mut<contexts::SwapchainContext<A>>| {
                                    let present_context =
                                        world.get_resource::<contexts::PresentContext>().unwrap();
                                    let core_context =
                                        world.get_resource::<contexts::CoreContext>().unwrap();
                                    if let Err(err) =
                                        swapchain_context.resize(extent, present_context, core_context)
                                    {
                                        tracing::error!(?err, ?extent, "Swapchain resize failed");
                                    }
                                },
                            );
                        }
                        RenderServerPacket::Recreate { size, handles } => {
                            world.resource_scope(
                                |world, mut swapchain_context: Mut<contexts::SwapchainContext<A>>| {
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
                    }
                }
                schedule.run(&mut world);
            }
            // shut down
            if let Some(core_context) = world.get_resource::<contexts::CoreContext>() {
                let _ = unsafe { core_context.device.get_handle().device_wait_idle() };
            }
            // drop all contexts here
            let present_context = world.remove_resource::<contexts::PresentContext>();
            let swapchain_context = world.remove_resource::<contexts::SwapchainContext<A>>();
            let core_context = world.remove_resource::<contexts::CoreContext>();
            drop(world);
            drop(present_context);
            drop(swapchain_context);
            drop(core_context);
        });
        Self {
            thread: Some(thread),
            drop_sender: Some(drop_sender),
            packet_sender,
        }
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
