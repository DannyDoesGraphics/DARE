use std::sync::Arc;
use dagal::winit;
use dagal::winit::window::WindowId;
use crate::render2::prelude as render;
use anyhow::Result;
use dagal::allocators::GPUAllocatorImpl;
use dagal::raw_window_handle::HasRawDisplayHandle;
use dagal::winit::window;

/// This app only exists to get the first window
pub struct App {
    window: Option<Arc<window::Window>>,
    render_server: Option<render::server::RenderServer>,
    configuration: render::create_infos::RenderContextConfiguration,
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = Arc::new(
            event_loop.create_window(
                window::WindowAttributes::default()
                    .with_title("DARE")
                    .with_resizable(true)
            ).unwrap()
        );
        println!("Yea");
        self.window = Some(window.clone());

        // If `render_server` is synchronous, handle it in a blocking thread
        let render_server_option = self.render_server.take();  // Take the render_server out
        let window_clone = window.clone();
        let config_clone = self.configuration.clone();

        futures::executor::block_on(async move {
            match render_server_option {
                None => {
                    let mut render_server = render::server::RenderServer::new(
                        render::create_infos::RenderContextCreateInfo {
                            rdh: window_clone.raw_display_handle().unwrap(),
                            configuration: config_clone,
                        }
                    );
                    // Call the synchronous blocking send function
                    render_server.send(render::RenderServerRequests::NewWindow(window_clone)).await.unwrap();
                }
                Some(mut rs) => {
                    // Use the already-existing render server
                    rs.send(render::RenderServerRequests::NewWindow(window_clone)).await.unwrap();
                }
            }
        });
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, window_id: WindowId, event: winit::event::WindowEvent) {
        use winit::event::WindowEvent;
        match event {
            WindowEvent::RedrawRequested => {
                if let Some(rs) = self.render_server.as_ref().cloned() {
                    futures::executor::block_on(async move {
                        let render = rs.send(render::RenderServerRequests::Render).await.unwrap();
                        render.notified().await;
                    });
                }
            },
            WindowEvent::CloseRequested => {
                if let Some(rs) = self.render_server.take() {
                    futures::executor::block_on(async move {
                        let render = rs.send(render::RenderServerRequests::Stop).await.unwrap();
                        render.notified().await;
                        drop(rs)
                    });
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) => {
                if let Some(rs) = self.render_server.as_ref().cloned() {
                    futures::executor::block_on(async move {
                        let render = rs.send(render::RenderServerRequests::NewSurface).await.unwrap();
                        render.notified().await;
                    });
                };
            }
            _ => {},
        }
    }
}

impl App {
    pub fn new(configuration: render::create_infos::RenderContextConfiguration) -> Result<Self> {
        Ok(Self {
            window: None,
            render_server: None,
            configuration,
        })
    }
}