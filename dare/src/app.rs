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
        self.window = Some(window.clone());

        // If `render_server` is synchronous, handle it in a blocking thread
        let render_server = self.render_server.take();  // Take the render_server out
        let window = window.clone();
        let config = self.configuration.clone();

        let render_server = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                match render_server {
                    None => {
                        let mut render_server = render::server::RenderServer::new(
                            render::create_infos::RenderContextCreateInfo {
                                rdh: window.raw_display_handle().unwrap(),
                                configuration: config,
                            }
                        );
                        // Call the synchronous blocking send function

                        render_server.create_surface(&window).await.unwrap();
                        render_server
                    }
                    Some(mut rs) => {
                        // Use the already-existing render server
                        rs.create_surface(&window).await.unwrap();
                        rs
                    }
                }
            })
        });
        self.render_server = Some(render_server);
    }

    fn window_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, window_id: WindowId, event: winit::event::WindowEvent) {
        use winit::event::WindowEvent;
        match event {
            WindowEvent::RedrawRequested => {
                if let Some(rs) = self.render_server.as_ref().cloned() {
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async move {
                            let render = rs.send(render::RenderServerRequests::Render).await.unwrap();
                            render.notified().await;
                        });
                    });
                } else {

                }
            },
            WindowEvent::CloseRequested => {
                if let Some(rs) = self.render_server.take() {
                    {
                        let rs = rs.clone();
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async move {
                                let render = rs.send(render::RenderServerRequests::Stop).await.unwrap();
                                render.notified().await;
                            });
                        });
                    }
                    while rs.strong_count() >= 3 {}
                    drop(rs);
                    tracing::warn!("Stopped?");
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) => {
                if let Some(rs) = self.render_server.as_ref().cloned() {
                    if let Some(window) = self.window.as_ref() {
                        if window.inner_size().width != 0 && window.inner_size().height != 0 {
                            tokio::task::block_in_place(|| {
                                tokio::runtime::Handle::current().block_on(async move {
                                    rs.create_surface(window).await.unwrap()
                                });
                            });
                        }
                    }
                };
            }
            _ => {},
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
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