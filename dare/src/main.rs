use dagal::winit;
use tracing_subscriber::FmtSubscriber;

mod app;
mod concurrent;
mod init_assets;
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
    let (input_send, _input_recv) = util::event::event_send::<dare_window::input::Input>();
    let (asset_manager_send, asset_manager_recv) = dare_assets::AssetManager::new(16);
    let (re_mesh_send, re_mesh_recv) = dare_extract::channel::<dare_assets::MeshHandle>();
    let (re_transform_send, re_transform_recv) = dare_extract::channel::<dare_physics::Transform>();
    let (re_bb_send, re_bb_recv) = dare_extract::channel::<dare_physics::BoundingBox>();
    let (re_camera_send, re_camera_recv) = dare_extract::channel::<dare_engine::Camera>();

    let (_engine_server, engine_client) =
        dare_engine::EngineServer::new(dare_engine::EngineServerConfig {
            assets_send: asset_manager_send,
            projections: dare_engine::EngineProjectionPlugins {
                send_mesh_handle: Some(re_mesh_send),
                send_transform: Some(re_transform_send),
                send_bounding_box: Some(re_bb_send),
                send_camera: Some(re_camera_send),
            },
        })
        .unwrap();
    let mut app = app::App::new(
        engine_client,
        input_send,
        asset_manager_recv,
        dare_render::RenderProjectionPlugins {
            recv_mesh_handle: Some(re_mesh_recv),
            recv_transform: Some(re_transform_recv),
            recv_bounding_box: Some(re_bb_recv),
            recv_camera: Some(re_camera_recv),
        },
    )
    .unwrap();
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).unwrap();
}
