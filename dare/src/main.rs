use crate::prelude::render;
use crate::prelude::render::{RenderServerPacket, RenderServerRequest};
use dagal::ash::vk;
use dagal::raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use dagal::winit;
use dagal::winit::platform::run_on_demand::EventLoopExtRunOnDemand;
use futures::executor;
use std::process::exit;
use std::sync::Arc;
use tracing_subscriber::FmtSubscriber;

mod app;
mod asset2;
mod concurrent;
mod engine;
mod physics;
mod prelude;
mod render2;
mod util;
mod window;

fn main() {
    tracy_client::Client::start();
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        eprintln!("The program panicked: {}", info);
        print!("Press Enter to exit...");
        std::io::stdout().flush().expect("Failed to flush stdout");
        let _ = std::io::stdin().read_line(&mut String::new());
    }));

    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_file(true)
        .with_line_number(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    // start the tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(32)
        .build()
        .unwrap();
    let asset_server = asset2::server::AssetServer::default();
    let (surface_link_send, surface_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (transform_link_send, transform_link_recv) =
        util::entity_linker::ComponentsLinker::default();
    let (bb_link_send, bb_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (texture_link_send, texture_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (name_link_send, name_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (rs_send, rs_recv) = tokio::sync::mpsc::unbounded_channel();
    let (es_sent, es_recv) = tokio::sync::mpsc::unbounded_channel();
    let (input_send, input_recv) = util::event::event_send::<window::input::Input>();
    let (window_send, window_recv) = tokio::sync::oneshot::channel::<window::WindowHandles>();
    // cross tokio-main thread communication
    let render_client = render2::server::RenderClient::new(rs_send, input_send);
    let engine_client = engine::server::EngineClient::new(es_sent);

    runtime.spawn(async move {
        // await, then spawn the render server
        let raw_handles = window_recv.await.unwrap();
        let render_server = render::server::RenderServer::new(
            tokio::runtime::Handle::current(),
            asset_server.clone(),
            rs_recv,
            input_recv,
            render::create_infos::RenderContextCreateInfo {
                raw_handles,
                configuration: render::create_infos::RenderContextConfiguration {
                    target_frames_in_flight: 3,
                    target_extent: vk::Extent2D {
                        width: 800,
                        height: 600,
                    },
                },
            },
            surface_link_recv,
            texture_link_recv,
            transform_link_recv,
            bb_link_recv,
            name_link_recv,
        )
        .await;
        let engine_server = engine::server::EngineServer::new(
            tokio::runtime::Handle::current(),
            es_recv,
            asset_server,
            &surface_link_send,
            &texture_link_send,
            &transform_link_send,
            &bb_link_send,
            &name_link_send,
        )
        .unwrap();
    });

    let mut app = app::App::new(render_client, engine_client, window_send).unwrap();
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).unwrap();
}
