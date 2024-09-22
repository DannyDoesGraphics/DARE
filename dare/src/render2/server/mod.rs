pub mod send_types;

use crate::render2::prelude::RenderServerRequests;
use crate::render2::server::send_types::RenderServerPacket;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use bevy_ecs::prelude::IntoSystemConfigs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RenderServer {
    /// Order a frame to be rendered
    frame_sender: tokio::sync::mpsc::Sender<tokio::sync::Notify>,
    /// Order a new window be created
    new_sender: tokio::sync::mpsc::Sender<RenderServerPacket>,
}

impl RenderServer {
    pub fn new(ci: super::render_context::RenderContextCreateInfo) -> Self {
        let (send, mut recv) = tokio::sync::mpsc::channel::<tokio::sync::Notify>(32);
        let (new_send, mut new_recv) = tokio::sync::mpsc::channel::<RenderServerPacket>(4);
        let shutdown_signal = Arc::new(AtomicBool::new(false));

        {
            // Render thread
            tokio::task::spawn(async move {
                let mut world = becs::World::new();
                world
                    .insert_resource(super::render_context::RenderContext::new(ci).unwrap());
                world
                    .insert_resource(super::frame_number::FrameCount::default());
                let mut schedule = becs::Schedule::default();
                schedule.add_systems(
                    super::present_system::present_system_begin::<GPUAllocatorImpl>
                );
                schedule.add_systems(
                    super::present_system::present_system_end::<GPUAllocatorImpl>.after(super::present_system::present_system_begin::<GPUAllocatorImpl>)
                );
                let mut stop_flag = false;
                while stop_flag == false {
                    match new_recv.try_recv() {
                        Ok(packet) => {
                            match packet.request {
                                RenderServerRequests::Render => {
                                    println!("Processing render");
                                    schedule.run(&mut world);
                                }
                                RenderServerRequests::NewWindow(window) => {
                                    println!("Processing window");
                                    let mut rx = world.resource::<super::render_context::RenderContext>();
                                    rx.build_surface(window).await.unwrap();
                                }
                                RenderServerRequests::NewSurface => {
                                    println!("Processing surface");
                                    let mut rx = world.resource::<super::render_context::RenderContext>();
                                    if let Some(window) = rx.inner.window_context.window.read().await.clone() {
                                        rx.build_surface(window).await.unwrap();
                                    } else {
                                        tracing::error!("NewSurface expected window, got None");
                                    }
                                }
                                RenderServerRequests::Stop => {
                                    println!("Processing stop");
                                    println!("I have become death");
                                    stop_flag = true;
                                    break
                                },
                            };
                            packet.callback.0.notify_waiters();
                        },
                        Err(e) => {}
                    }
                }
            });
        }
        Self {
            frame_sender: send,
            new_sender: new_send,
        }
    }

    pub async fn send(&self, request: RenderServerRequests) -> Result<Arc<tokio::sync::Notify>> {
        let notify = Arc::new(tokio::sync::Notify::new());
        println!("Requested {:?}", request);
        self.new_sender.send(
            RenderServerPacket {
                callback: send_types::Callback(notify.clone()),
                request,
            }
        ).await?;
        Ok(notify)
    }

    pub fn blocking_send(&self, request: RenderServerRequests) -> Result<Arc<tokio::sync::Notify>> {
        let notify = Arc::new(tokio::sync::Notify::new());
        self.new_sender.blocking_send(
            RenderServerPacket {
                callback: send_types::Callback(notify.clone()),
                request,
            }
        )?;
        Ok(notify)
    }
}