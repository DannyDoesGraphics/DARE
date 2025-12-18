use dagal::winit;
use tracing_subscriber::FmtSubscriber;

mod app;
mod concurrent;
mod engine;
mod util;

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
    let _client = tracy_client::Client::start();
    let (es_sent, es_recv) = std::sync::mpsc::channel::<()>();
    let (input_send, _input_recv) = util::event::event_send::<dare_window::input::Input>();
    let engine_client = engine::server::EngineClient::new(es_sent);

    let _engine_server = engine::server::EngineServer::new(es_recv).unwrap();
    let mut app = app::App::new(engine_client, input_send).unwrap();
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).unwrap();
}
