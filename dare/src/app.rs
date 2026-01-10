use anyhow::Result;
use dagal::ash::vk;
use dagal::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use dagal::winit;
use dagal::winit::window;
use dagal::winit::window::WindowId;
use dare_render::{self, RenderServerPacket};
use std::sync::Arc;

/// This app only exists to get the first window
pub struct App {
    window: Option<Arc<window::Window>>,
    engine_client: dare_engine::EngineClient,
    render_server: Option<dare_render::RenderServer>,
    input_sender: crate::util::event::EventSender<dare_window::input::Input>,
    last_position: Option<glam::Vec2>,
    modifier_state: winit::keyboard::ModifiersState,
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
            self.ensure_render_server();
        } else {
            self.ensure_render_server();
            if let Some(window) = &self.window {
                self.send_recreate(window);
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: WindowId,
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
                self.render_server.take();
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                if let Some(window) = self.window.as_ref()
                    && window.inner_size().width != 0
                    && window.inner_size().height != 0
                {
                    self.send_resize(window);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(window) = self.window.as_ref() {
                    let position = position.to_logical(window.scale_factor());
                    let position = glam::Vec2::new(position.x, position.y);
                    let dp: Option<glam::Vec2> = self
                        .last_position
                        .as_ref()
                        .and_then(|last_position| Some(position - last_position));
                    self.last_position = Some(position);
                    if let Some(dp) = dp {
                        let _ = self
                            .input_sender
                            .send(dare_window::input::Input::MouseDelta(dp));
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.last_position = None;
            }
            WindowEvent::ModifiersChanged(modifier) => {
                self.modifier_state = modifier.state();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed()
                    && !event.repeat
                    && (self.modifier_state.control_key() || self.modifier_state.super_key())
                    && event.physical_key
                        == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyO)
                {
                    // open file dialog
                    let path = rfd::FileDialog::new()
                        .add_filter("gltf", &["gltf", "glb"])
                        .set_title("Gltf file to load")
                        .pick_files();
                    println!("Selected file: {:?}", path);
                }

                let _ = self
                    .input_sender
                    .send(dare_window::input::Input::KeyEvent(event));
            }
            WindowEvent::MouseInput {
                device_id: _,
                state,
                button,
            } => {
                let _ = self
                    .input_sender
                    .send(dare_window::input::Input::MouseButton { button, state });
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Err(e) = self.engine_client.tick() {
            tracing::error!("Engine tick error: {}", e);
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

impl App {
    pub fn new(
        engine_client: dare_engine::EngineClient,
        input_sender: crate::util::event::EventSender<dare_window::input::Input>,
    ) -> Result<Self> {
        loop {
            let paths: Option<Vec<std::path::PathBuf>> = rfd::FileDialog::new()
                .add_filter("gltf", &["gltf", "glb"])
                .set_title("Gltf file to load")
                .pick_files();
            if let Some(paths) = paths {
                let all_loaded: bool =
                    paths
                        .iter()
                        .all(|path| match engine_client.load_gltf(path.clone()) {
                            Ok(_) => true,
                            Err(err) => {
                                tracing::error!("Failed to load gltf file: {}", err);
                                false
                            }
                        });
                if all_loaded {
                    break;
                }
            }
        }
        Ok(Self {
            window: None,
            engine_client,
            render_server: None,
            input_sender,
            last_position: None,
            modifier_state: winit::keyboard::ModifiersState::default(),
            last_dt: std::time::Instant::now(),
        })
    }
}

impl App {
    fn ensure_render_server(&mut self) {
        if self.render_server.is_some() {
            return;
        }
        let window = match &self.window {
            Some(window) => window,
            None => return,
        };
        let extent = Self::window_extent(window.as_ref());
        let handles = Self::window_handles(window.as_ref());
        self.render_server = Some(dare_render::RenderServer::new(extent, handles));
    }

    fn window_extent(window: &window::Window) -> vk::Extent2D {
        vk::Extent2D {
            width: window.inner_size().width,
            height: window.inner_size().height,
        }
    }

    fn window_handles(window: &window::Window) -> dare_window::WindowHandles {
        let window_handle = window
            .window_handle()
            .expect("window handle unavailable")
            .as_raw();
        let display_handle = window
            .display_handle()
            .expect("display handle unavailable")
            .as_raw();
        dare_window::WindowHandles {
            raw_window_handle: Arc::new(window_handle),
            raw_display_handle: Arc::new(display_handle),
        }
    }

    fn send_resize(&self, window: &window::Window) {
        if let Some(server) = &self.render_server {
            let extent = Self::window_extent(window);
            if let Err(err) = server
                .packet_sender
                .send(RenderServerPacket::Resize(extent))
            {
                tracing::warn!(?extent, ?err, "Failed to send resize packet");
            }
        }
    }

    fn send_recreate(&self, window: &window::Window) {
        if let Some(server) = &self.render_server {
            let size = Self::window_extent(window);
            let handles = Self::window_handles(window);
            let packet = RenderServerPacket::Recreate { size, handles };
            if let Err(err) = server.packet_sender.send(packet) {
                tracing::warn!(?size, ?err, "Failed to send recreate packet");
            }
        }
    }
}
