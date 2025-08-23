use crate::engine;
use crate::prelude as dare;
use crate::prelude::render::RenderServerRequest;
use crate::render2::prelude as render;
use anyhow::Result;
use dagal::raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use dagal::winit;
use dagal::winit::window;
use dagal::winit::window::WindowId;
use dagal::wsi::WindowDimensions;
use futures::FutureExt;
use std::sync::Arc;

/// This app only exists to get the first window
pub struct App {
    window_send: Option<tokio::sync::oneshot::Sender<dare::window::WindowHandles>>,
    window: Option<Arc<window::Window>>,
    engine_client: engine::server::engine_server::EngineClient,
    render_client: render::server::RenderClient,
    last_position: Option<glam::Vec2>,
    last_dt: std::time::Instant,
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_none() {
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
            self.window_send.take().map(|send| {
                send.send(dare::window::WindowHandles {
                    raw_window_handle: Arc::new(window.raw_window_handle().unwrap()),
                    raw_display_handle: Arc::new(window.raw_display_handle().unwrap()),
                })
            });
            self.render_client
                .send_blocking(RenderServerRequest::SurfaceUpdate {
                    dimensions: Some((
                        self.window.as_ref().unwrap().width(),
                        self.window.as_ref().unwrap().height(),
                    )),
                    raw_handles: None,
                })
                .unwrap();
        } else {
            self.render_client
                .send_blocking(RenderServerRequest::SurfaceUpdate {
                    dimensions: Some((
                        self.window.as_ref().unwrap().width(),
                        self.window.as_ref().unwrap().height(),
                    )),
                    raw_handles: None,
                })
                .unwrap();
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
                // check if there is a valid window to render to
                if self
                    .window
                    .as_ref()
                    .map(|window| window.inner_size().width != 0 && window.inner_size().height != 0)
                    .unwrap_or(false)
                {
                    let window_clone = self.window.clone();
                    let current_t = std::time::Instant::now();
                    let last_dt = self.last_dt;

                    self.render_client
                        .send_blocking(RenderServerRequest::RenderStart)
                        .unwrap();

                    // Update the window title if needed
                    if let Some(window) = window_clone {
                        window.set_title(&format!(
                            "DARE | micro-seconds: {}",
                            current_t.duration_since(last_dt).as_millis()
                        ));
                    }

                    // Update the last_dt here
                    self.last_dt = current_t;
                }
            }
            WindowEvent::CloseRequested => {
                // Spawn instead of blocking the current thread
                unsafe {
                    // SAFETY: do not care if this fails
                    self.render_client
                        .send_blocking(RenderServerRequest::Stop)
                        .unwrap_err_unchecked();
                }
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                if let Some(window) = self.window.as_ref() {
                    if window.inner_size().width != 0 && window.inner_size().height != 0 {
                        self.render_client
                            .send_blocking(RenderServerRequest::SurfaceUpdate {
                                dimensions: Some((
                                    window.inner_size().width,
                                    window.inner_size().height,
                                )),
                                raw_handles: None,
                            })
                            .unwrap();
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(window) = self.window.as_ref() {
                    let position = position.to_logical(window.scale_factor());
                    let position = glam::Vec2::new(position.x, position.y);
                    let dp: Option<glam::Vec2> = self
                        .last_position
                        .as_ref()
                        .map(|last_position| Some(position - last_position))
                        .flatten();
                    self.last_position = Some(position);
                    if let Some(dp) = dp {
                        self.render_client
                            .input_send()
                            .send(dare::window::input::Input::MouseDelta(dp))
                            .unwrap();
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.last_position = None;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.render_client
                    .input_send()
                    .send(dare::window::input::Input::KeyEvent(event))
                    .unwrap();
            }
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                self.render_client
                    .input_send()
                    .send(dare::window::input::Input::MouseButton { button, state })
                    .unwrap();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Err(_) = self.engine_client.tick() {
            //eprintln!("Engine tick error: {}", e);
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

impl App {
    pub fn new(
        render_client: render::server::RenderClient,
        engine_client: engine::server::EngineClient,
        window_send: tokio::sync::oneshot::Sender<dare::window::WindowHandles>,
    ) -> Result<Self> {
        Ok(Self {
            window_send: Some(window_send),
            window: None,
            engine_client,
            render_client,
            last_position: None,
            last_dt: std::time::Instant::now(),
        })
    }
}
