use dagal::ash::vk;
use dagal::winit;
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

#[tokio::main(flavor = "multi_thread")]
async fn main() {
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
    /*
    let event_loop = window::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(window::event_loop::ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();

    let bevy_loop = World::new();
    */
    let mut app = app::App::new(render2::prelude::create_infos::RenderContextConfiguration {
        target_frames_in_flight: 2,
        target_extent: vk::Extent2D {
            width: 800,
            height: 600,
        },
    })
    .unwrap();
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut app).unwrap();
}
