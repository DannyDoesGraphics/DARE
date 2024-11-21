pub mod send_types;

use crate::prelude as dare;
use crate::render2::prelude as render;
use crate::render2::server::send_types::RenderServerPacket;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use bevy_ecs::prelude::IntoSystemConfigs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::winit;
use derivative::Derivative;
use std::cmp::PartialEq;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::mpsc::error::TryRecvError;

#[derive(Debug)]
pub struct RenderServerInner {
    input_send: dare::util::event::EventSender<dare::winit::input::Input>,
    thread: tokio::task::JoinHandle<()>,
    ir_send: crossbeam_channel::Sender<render::InnerRenderServerRequest>,
    /// Order a new window be created
    new_sender: tokio::sync::mpsc::UnboundedSender<RenderServerPacket>,
}
impl Drop for RenderServerInner {
    fn drop(&mut self) {
        while !self.thread.is_finished() {}
        tracing::trace!("RENDER SERVER STOPPED (2)");
    }
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct RenderServer {
    /// stored assets
    #[derivative(Debug = "ignore")]
    asset_server: dare::asset2::server::AssetServer,
    /// inner
    inner: Arc<RenderServerInner>,
    /// A ref to render context
    render_context: render::contexts::RenderContext,
}
#[derive(becs::Resource)]
pub struct IrRecv(pub(crate) crossbeam_channel::Receiver<render::InnerRenderServerRequest>);

#[derive(becs::Resource, Clone)]
pub struct IrSend(pub(crate) crossbeam_channel::Sender<render::InnerRenderServerRequest>);

impl RenderServer {
    pub fn input_send(&self) -> &dare::util::event::EventSender<dare::winit::input::Input> {
        &self.inner.input_send
    }

    pub fn new(ci: super::render_context::RenderContextCreateInfo) -> Self {
        let (new_send, mut new_recv) = tokio::sync::mpsc::unbounded_channel::<RenderServerPacket>();
        let asset_server = dare::asset2::server::AssetServer::default();
        let render_context = super::render_context::RenderContext::new(ci).unwrap();
        let (ir_send, ir_recv) = crossbeam_channel::unbounded::<render::InnerRenderServerRequest>();
        let mut world = dare::util::world::World::new();
        let input_send = world.add_event::<dare::winit::input::Input>();
        let thread = {
            let render_context = render_context.clone();
            let rt = dare::concurrent::BevyTokioRunTime::default();
            let asset_server = asset_server.clone();

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
                    world.insert_resource(super::resources::mesh_buffer::MeshBuffer {
                        uploaded_hash: 0,
                        growable_buffer: render::util::GrowableBuffer::new(
                            dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                                device: render_context.inner.device.clone(),
                                name: Some(String::from("Mesh buffer")),
                                allocator: &mut allocator,
                                size: (size_of::<render::c::CSurface>() * 128) as vk::DeviceSize,
                                memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                                usage_flags: vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS | vk::BufferUsageFlags::STORAGE_BUFFER,
                            }
                        ).unwrap(),
                        mesh_container: dare_containers::prelude::slot_map::SlotMap::default(),
                        external_id_mapping: Default::default(),
                    });
                }
                world.insert_resource(render_context);
                world.insert_resource(super::frame_number::FrameCount::default());
                world.insert_resource(rt);
                world.insert_resource(asset_server.clone());
                world.insert_resource(render::render_assets::server::RenderAssetServer::new(
                    asset_server.clone(),
                ));
                world.insert_resource(render::components::camera::Camera::default());
                world.insert_resource(IrRecv(ir_recv));
                world.insert_resource(render::render_assets::RenderAssetsStorage::<
                    render::render_assets::components::RenderBuffer<GPUAllocatorImpl>,
                >::default());
                world.insert_resource(super::systems::delta_time::DeltaTime::default());
                let mut schedule = becs::Schedule::default();
                schedule.add_systems(super::systems::delta_time::delta_time_update);
                schedule.add_systems(super::components::camera::camera_system);
                schedule.add_systems(
                    render::render_assets::server::process_asset_relations_incoming_system,
                );
                // rendering
                schedule.add_systems(super::present_system::present_system_begin);
                let mut stop_flag = false;
                while stop_flag == false {
                    match new_recv.recv().await {
                        Some(packet) => {
                            match packet.request {
                                render::RenderServerNoCallbackRequest::Render => {
                                    schedule.run(&mut world);
                                }
                                render::RenderServerNoCallbackRequest::Stop => {
                                    let mut shutdown_schedule = becs::Schedule::default();
                                    shutdown_schedule.add_systems(render::systems::shutdown_system::render_server_shutdown_system);
                                    shutdown_schedule.run(&mut world);
                                    stop_flag = true;
                                }
                            };
                            packet.callback.0.notify_waiters();
                        }
                        None => {}
                    }
                }
                tracing::trace!("Stopping render server");
                // drop world
                drop(world);
                tracing::trace!("RENDER SERVER STOPPED");
            })
        };
        *render_context.inner.render_thread.write().unwrap() = Some(thread.abort_handle());
        Self {
            render_context,
            asset_server,
            inner: Arc::new(RenderServerInner {
                new_sender: new_send,
                thread,
                ir_send,
                input_send,
            }),
        }
    }

    pub fn send_inner(&self, request: render::InnerRenderServerRequest) {
        self.inner.ir_send.send(request).unwrap();
    }

    pub fn get_inner_send(&self) -> IrSend {
        IrSend(self.inner.ir_send.clone())
    }

    pub async fn send(
        &self,
        request: render::RenderServerNoCallbackRequest,
    ) -> Result<Arc<tokio::sync::Notify>> {
        let notify = Arc::new(tokio::sync::Notify::new());
        self.inner.new_sender.send(RenderServerPacket {
            callback: send_types::Callback(notify.clone()),
            request,
        })?;
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
        self.inner.new_sender.send(RenderServerPacket {
            callback: send_types::Callback(notify.clone()),
            request,
        })?;
        Ok(notify)
    }

    pub fn create_surface(&self, window: &winit::window::Window) -> Result<()> {
        self.render_context.inner.window_context.build_surface(
            render::create_infos::SurfaceContextCreateInfo {
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
            },
        )?;
        Ok(())
    }

    pub fn strong_count(&self) -> usize {
        self.render_context.strong_count()
    }

    pub fn asset_server(&self) -> dare::asset2::server::AssetServer {
        self.asset_server.clone()
    }
}
