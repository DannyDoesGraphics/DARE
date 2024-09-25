pub mod send_types;

use crate::render2::prelude as render;
use crate::render2::server::send_types::RenderServerPacket;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use bevy_ecs::prelude::IntoSystemConfigs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::winit;
use std::cmp::PartialEq;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc::error::TryRecvError;

#[derive(Debug, Clone)]
pub struct RenderServer {
    /// A ref to render context
    render_context: render::contexts::RenderContext,
    /// Order a new window be created
    new_sender: tokio::sync::mpsc::Sender<RenderServerPacket>,
}

impl RenderServer {
    pub fn new(ci: super::render_context::RenderContextCreateInfo) -> Self {
        let (new_send, mut new_recv) = tokio::sync::mpsc::channel::<RenderServerPacket>(4);
        let render_context = super::render_context::RenderContext::new(ci).unwrap();

        {
            let render_context = render_context.clone();
            // Render thread
            tokio::task::spawn_blocking(move || {
                let mut world = becs::World::new();
                world.insert_resource(render_context);
                world.insert_resource(super::frame_number::FrameCount::default());
                let mut schedule = becs::Schedule::default();
                schedule.add_systems(super::present_system::present_system_begin);
                schedule.add_systems(
                    super::present_system::present_system_end
                        .after(super::present_system::present_system_begin),
                );
                let mut stop_flag = false;
                while stop_flag == false {
                    match new_recv.try_recv() {
                        Ok(packet) => {
                            match packet.request {
                                render::RenderServerNoCallbackRequest::Render => {
                                    println!("Processing render");
                                    schedule.run(&mut world);
                                }
                                render::RenderServerNoCallbackRequest::Stop => {
                                    println!("Processing stop");
                                    stop_flag = true;
                                }
                            };
                            packet.callback.0.notify_waiters();
                        }
                        Err(e) => match e {
                            TryRecvError::Disconnected => {
                                stop_flag = true;
                                break;
                            }
                            _ => {}
                        },
                    }
                }
                tracing::trace!("RENDER SERVER STOPPED");
            });
        }
        Self {
            new_sender: new_send,
            render_context,
        }
    }

    pub async fn send(
        &self,
        request: render::RenderServerNoCallbackRequest,
    ) -> Result<Arc<tokio::sync::Notify>> {
        let notify = Arc::new(tokio::sync::Notify::new());
        println!("Requested {:?}", request);
        self.new_sender
            .send(RenderServerPacket {
                callback: send_types::Callback(notify.clone()),
                request,
            })
            .await?;
        Ok(notify)
    }

    pub fn blocking_send(
        &self,
        request: render::RenderServerNoCallbackRequest,
    ) -> Result<Arc<tokio::sync::Notify>> {
        match &request {
            render::RenderServerNoCallbackRequest::Stop => {}
            _ => {}
        }
        let notify = Arc::new(tokio::sync::Notify::new());
        self.new_sender.blocking_send(RenderServerPacket {
            callback: send_types::Callback(notify.clone()),
            request,
        })?;
        Ok(notify)
    }

    pub async fn create_surface(&self, window: &winit::window::Window) -> Result<()> {
        self.render_context
            .inner
            .window_context
            .build_surface(render::create_infos::SurfaceContextCreateInfo {
                instance: &self.render_context.inner.instance,
                physical_device: &self.render_context.inner.physical_device,
                allocator: self.render_context.inner.allocator.clone(),
                window,
                frames_in_flight: Some(
                    self.render_context
                        .inner
                        .configuration
                        .target_frames_in_flight,
                ),
            })
            .await?;
        Ok(())
    }

    pub fn strong_count(&self) -> usize {
        self.render_context.strong_count()
    }
}
