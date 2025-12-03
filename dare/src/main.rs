use dagal::winit;
use tracing_subscriber::FmtSubscriber;

mod app;
mod asset;
mod asset_system;
mod concurrent;
mod engine;
mod physical_resource;
mod physics;
mod prelude;
mod render;
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
        .build()
        .unwrap();
    let asset_server = asset::server::AssetServer::default();
    let (surface_link_send, _surface_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (transform_link_send, _transform_link_recv) =
        util::entity_linker::ComponentsLinker::default();
    let (bb_link_send, _bb_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (texture_link_send, _texture_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (name_link_send, _name_link_recv) = util::entity_linker::ComponentsLinker::default();
    let (es_sent, es_recv) = std::sync::mpsc::channel::<()>();
    let (input_send, _input_recv) = util::event::event_send::<window::input::Input>();
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
    let mut app = app::App::new(engine_client, input_send).unwrap();
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).unwrap();
}
