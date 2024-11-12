use crate::engine;
use crate::prelude as dare;
use crate::render2::prelude as render;
use anyhow::Result;
use dagal::allocators::GPUAllocatorImpl;
use dagal::raw_window_handle::HasRawDisplayHandle;
use dagal::winit;
use dagal::winit::window;
use dagal::winit::window::WindowId;
use std::sync::Arc;

/// This app only exists to get the first window
pub struct App {
    window: Option<Arc<window::Window>>,
    engine_server: Option<engine::server::engine_server::EngineServer>,
    render_server: Option<render::server::RenderServer>,
    configuration: render::create_infos::RenderContextConfiguration,
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    window::WindowAttributes::default()
                        .with_title("DARE")
                        .with_resizable(true),
                )
                .unwrap(),
        );
        self.window = Some(window.clone());

        let window = window.clone();
        let config = self.configuration.clone();

        tokio::task::block_in_place(|| {
            match self.render_server.as_mut() {
                None => {
                    // render server does not exist yet
                    let mut render_server = render::server::RenderServer::new(
                        render::create_infos::RenderContextCreateInfo {
                            rdh: window.raw_display_handle().unwrap(),
                            configuration: config,
                        },
                    );
                    // Call the synchronous blocking send function
                    render_server.create_surface(&window).unwrap();
                    self.render_server = Some(render_server);
                }
                Some(rs) => {
                    rs.create_surface(&window).unwrap();
                }
            };
        });
        if self.engine_server.is_none() {
            self.engine_server = Some(
                engine::server::EngineServer::new(
                    self.render_server.as_ref().cloned().unwrap().asset_server(),
                    self.render_server.as_ref().unwrap().get_inner_send(),
                )
                .unwrap(),
            );
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent;
        match event {
            WindowEvent::RedrawRequested => {
                if let (Some(rs), Some(es)) =
                    (self.render_server.as_ref(), self.engine_server.as_ref())
                {
                    tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async move {
                            let render = rs
                                .send(render::RenderServerNoCallbackRequest::Render)
                                .await
                                .unwrap();
                            render.notified().await;
                            es.tick().await.unwrap();
                        });
                    });
                    if let Some(window) = self.window.as_ref() {
                        window.set_title(&format!("DARE | FPS: {}", 1));
                    }
                } else {
                }
            }
            WindowEvent::CloseRequested => {
                if let Some(rs) = self.render_server.take() {
                    {
                        let rs = rs.clone();
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async move {
                                let render = rs
                                    .send(render::RenderServerNoCallbackRequest::Stop)
                                    .await
                                    .unwrap();
                                render.notified().await;
                            });
                        });
                    }
                    // drop engine server first
                    drop(self.engine_server.take());
                    tracing::trace!("Dropped engine server");
                    drop(rs);
                    tracing::trace!("Dropped render server");
                    tracing::warn!("Stopped?");
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(_) => {
                if let Some(rs) = self.render_server.as_ref().cloned() {
                    if let Some(window) = self.window.as_ref() {
                        if window.inner_size().width != 0 && window.inner_size().height != 0 {
                            tokio::task::block_in_place(|| rs.create_surface(window).unwrap());
                        }
                    }
                };
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        drop(self.engine_server.take());
    }
}

impl App {
    pub fn new(configuration: render::create_infos::RenderContextConfiguration) -> Result<Self> {
        Ok(Self {
            window: None,
            engine_server: None,
            render_server: None,
            configuration,
        })
    }
}
