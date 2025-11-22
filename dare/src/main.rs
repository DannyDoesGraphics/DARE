use dagal::ash::vk;
use dagal::winit;
use tracing_subscriber::FmtSubscriber;

mod app;
mod asset;
mod concurrent;
mod engine;
mod physical_resource;
mod physics;
mod prelude;
mod render;
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
        .build()
        .unwrap();
    let asset_server = asset::server::AssetServer::default();
    let (surface_link_send, surface_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (transform_link_send, transform_link_recv) =
        util::entity_linker::ComponentsLinker::default();
    let (bb_link_send, bb_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (texture_link_send, texture_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (name_link_send, name_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (rs_send, rs_recv) = tokio::sync::mpsc::unbounded_channel();
    let (es_sent, es_recv) = std::sync::mpsc::channel::<()>();
    let (input_send, input_recv) = util::event::event_send::<window::input::Input>();
    let (window_send, window_recv) = tokio::sync::oneshot::channel::<window::WindowHandles>();
    // cross tokio-main thread communication
    let render_client = render::server::RenderClient::new(rs_send, input_send);
    let engine_client = engine::server::EngineClient::new(es_sent);

    let _engine_server = engine::server::EngineServer::new(
        runtime.handle().clone(),
        es_recv,
        asset_server.clone(),
        &surface_link_send,
        &texture_link_send,
        &transform_link_send,
        &bb_link_send,
        &name_link_send,
    )
    .unwrap();
    runtime.spawn(async move {
        // await, then spawn the render server
        let raw_handles = window_recv.await.unwrap();
        let render_server = render::server::RenderServer::new(
            tokio::runtime::Handle::current(),
            asset_server.clone(),
            rs_recv,
            input_recv,
            render::contexts::ContextsCreateInfo {
                raw_handles,
                configuration: render::contexts::ContextsConfiguration {
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
    });

    let mut app = app::App::new(render_client, engine_client, window_send).unwrap();
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).unwrap();
}
