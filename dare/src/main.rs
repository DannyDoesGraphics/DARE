use tracing_subscriber::FmtSubscriber;
use dagal::ash::vk;
use dagal::winit;

mod app;
mod render2;
mod physics;
mod prelude;
mod asset;
mod util;

#[tokio::main]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .with_file(true)
        .with_line_number(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    /*
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
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
