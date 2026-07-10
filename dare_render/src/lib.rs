use dagal::ash::vk;
use dare_ecs::SubApp;
use dare_window::{Window, WindowHandles};

pub mod components;
mod contexts;
pub mod extract;
mod frame;
mod plugin;
mod render_resources;
pub mod snapshot;
mod systems;
mod timer;
mod transfer_belt;
pub use plugin::{
    Camera, CameraPlugin, CameraUpdate, CullPlugin, FlyController, FlyControllerPlugin,
    RenderContext, RenderMode, RenderModePlugin, RenderPlugin, RenderPluginConfig,
    RenderSubAppLabel, VisibleMeshList,
};

#[derive(Debug)]
pub struct RenderServerSpawnConfig {
    pub frames_in_flight: usize,
    pub transfer_buffer_size: u64,
    pub max_transfers: u64,
    /// Reserved for future asset upload on the render thread.
    pub render_sub_app: SubApp,
}

#[derive(Debug)]
pub enum RenderPacket {
    Window(Window),
    FrameStart,
    /// Last message before senders are dropped.
    Drop,
}

/// Main-thread handle to the render thread. Dropping this sends shutdown and joins.
#[derive(Debug)]
pub struct RenderClient {
    packet_sender: crossbeam_channel::Sender<RenderPacket>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl RenderClient {
    pub fn send_window(&self, window: Window) -> anyhow::Result<()> {
        Ok(self.packet_sender.send(RenderPacket::Window(window))?)
    }

    pub fn frame_render_start(&self) -> anyhow::Result<()> {
        Ok(self.packet_sender.send(RenderPacket::FrameStart)?)
    }
}

impl RenderClient {
    /// Takes the render sub-app from the main [`dare_ecs::App`] during plugin cleanup.
    pub fn spawn(spawn: RenderServerSpawnConfig) -> Self {
        let gpu_config = RenderGpuConfig {
            frames_in_flight: spawn.frames_in_flight,
            transfer_buffer_size: spawn.transfer_buffer_size,
            max_transfers: spawn.max_transfers,
        };
        let (packet_sender, packet_receiver) = crossbeam_channel::unbounded::<RenderPacket>();

        let thread = std::thread::Builder::new()
            .name(String::from("Render thread"))
            .spawn(move || {
                let mut render = spawn.render_sub_app;

                loop {
                    let Ok(packet) = packet_receiver.recv() else {
                        break;
                    };
                    match packet {
                        RenderPacket::Window(window) => {
                            apply_window(&mut render, gpu_config, window)
                        }
                        RenderPacket::FrameStart => tick_render_frame(&mut render),
                        RenderPacket::Drop => break,
                    }
                }

                teardown_gpu(&mut render);
            })
            .unwrap();

        Self {
            packet_sender,
            thread: Some(thread),
        }
    }
}

impl Drop for RenderClient {
    fn drop(&mut self) {
        let _ = self.packet_sender.send(RenderPacket::Drop);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("render thread panicked during shutdown");
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RenderGpuConfig {
    frames_in_flight: usize,
    transfer_buffer_size: u64,
    max_transfers: u64,
}

fn gpu_ready(render: &SubApp) -> bool {
    type A = dagal::allocators::GPUAllocatorImpl;
    render
        .world()
        .get_non_send::<contexts::RenderGpu<A>>()
        .is_some()
}

fn tick_render_frame(render: &mut SubApp) {
    if !render
        .world()
        .get_resource::<Window>()
        .is_some_and(|w| w.is_valid())
        || !gpu_ready(render)
    {
        return;
    }
    render.tick();
}

fn apply_window(render: &mut SubApp, gpu_config: RenderGpuConfig, window: Window) {
    let previous = render
        .world()
        .get_resource::<Window>()
        .cloned()
        .unwrap_or(Window::None);

    render.world_mut().insert_resource(window.clone());

    if !window.is_valid() {
        teardown_gpu(render);
        return;
    }

    sync_gpu(render, gpu_config, &window, &previous);
}

fn sync_gpu(render: &mut SubApp, gpu_config: RenderGpuConfig, window: &Window, previous: &Window) {
    let Window::Window { physical_size, .. } = window else {
        return;
    };
    let extent = vk::Extent2D {
        width: physical_size.0,
        height: physical_size.1,
    };

    if !gpu_ready(render) {
        bootstrap_gpu(render, gpu_config, window);
        return;
    }

    let handles_changed = match (previous, window) {
        (Window::None, Window::Window { .. }) => true,
        (
            Window::Window {
                raw_window_handle: prev_wh,
                raw_display_handle: prev_dh,
                ..
            },
            Window::Window {
                raw_window_handle: wh,
                raw_display_handle: dh,
                ..
            },
        ) => prev_wh != wh || prev_dh != dh,
        _ => false,
    };
    let size_changed = !previous.same_surface(window);

    if handles_changed {
        apply_recreate(render, extent, window.handles());
    } else if size_changed {
        apply_resize(render, extent);
    }
}

fn bootstrap_gpu(render: &mut SubApp, gpu_config: RenderGpuConfig, window: &Window) {
    type A = dagal::allocators::GPUAllocatorImpl;

    let Window::Window { physical_size, .. } = window else {
        return;
    };
    let extent = vk::Extent2D {
        width: physical_size.0,
        height: physical_size.1,
    };

    render.world_mut().insert_resource(timer::Timer {
        last_recorded: None,
    });

    let (mut core_context, surface) = contexts::CoreContext::new(&window.handles()).unwrap();
    let swapchain_context =
        contexts::SwapchainContext::<A>::new(surface, extent, &core_context).unwrap();
    let mut present_context =
        contexts::PresentContext::new(&core_context, gpu_config.frames_in_flight).unwrap();
    present_context
        .rebuild_present_semaphores(&core_context.device, swapchain_context.image_count())
        .unwrap();
    let transfer_manager = transfer_belt::TransferManager::<A>::new(
        core_context.queues.take_transfer().unwrap(),
        core_context.allocator.clone(),
        gpu_config.transfer_buffer_size,
        gpu_config.max_transfers,
    )
    .unwrap();
    let transfer_pool = transfer_manager.get_transfer_pool();
    let gpu = contexts::RenderGpu {
        core: core_context,
        present: present_context,
        swapchain: swapchain_context,
        transfer: transfer_manager,
        transfer_pool,
    };

    render.world_mut().insert_non_send(gpu);

    tracing::info!(?extent, "Render surface ready");
}

fn apply_resize(render: &mut SubApp, extent: vk::Extent2D) {
    type A = dagal::allocators::GPUAllocatorImpl;
    let mut gpu = render
        .world_mut()
        .get_non_send_mut::<contexts::RenderGpu<A>>()
        .expect("RenderGpu missing during resize");
    if let Err(err) = gpu.resize(extent) {
        tracing::error!(?err, ?extent, "Swapchain resize failed");
    }
}

fn apply_recreate(render: &mut SubApp, size: vk::Extent2D, handles: WindowHandles) {
    type A = dagal::allocators::GPUAllocatorImpl;
    let mut gpu = render
        .world_mut()
        .get_non_send_mut::<contexts::RenderGpu<A>>()
        .expect("RenderGpu missing during recreate");
    if let Err(err) = gpu.recreate(size, handles) {
        tracing::error!(?err, ?size, "Swapchain recreate failed");
    }
}

fn teardown_gpu(render: &mut SubApp) {
    type A = dagal::allocators::GPUAllocatorImpl;

    if !gpu_ready(render) {
        return;
    }

    let world = render.world_mut();
    world.remove_resource::<timer::Timer>();

    if let Some(gpu) = world.remove_non_send::<contexts::RenderGpu<A>>() {
        gpu.shutdown();
    }
}
