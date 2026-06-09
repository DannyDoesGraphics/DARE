use dagal::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use dagal::winit;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::WindowPluginConfig;
use crate::input::Input;
use crate::input_log::InputLog;
use crate::messages::WindowMessage;
use crate::window::{Window, WinitWindow};

pub struct WinitApp {
    pub app: dare_ecs::App,
    config: WindowPluginConfig,
    last_position: Option<glam::Vec2>,
    modifier_state: winit::keyboard::ModifiersState,
    awaiting_destroyed: bool,
}

impl WinitApp {
    pub fn new(app: dare_ecs::App, config: WindowPluginConfig) -> Self {
        Self {
            app,
            config,
            last_position: None,
            modifier_state: winit::keyboard::ModifiersState::default(),
            awaiting_destroyed: false,
        }
    }

    fn push_input(&mut self, event: Input) {
        if let Some(mut log) = self.app.world_mut().get_resource_mut::<InputLog>() {
            log.push(event);
        }
    }

    fn send(&mut self, message: WindowMessage) {
        let _ = self.app.world_mut().write_message(message);
    }

    fn begin_close(&mut self) {
        tracing::info!("window close requested");
        self.awaiting_destroyed = true;
        self.send(WindowMessage::CloseRequested);
        self.app.world_mut().insert_resource(Window::None);
        self.app.tick();

        if let Some(winit_window) = self.app.world_mut().remove_resource::<WinitWindow>() {
            drop(winit_window);
        }
    }
}

impl Deref for WinitApp {
    type Target = dare_ecs::App;
    fn deref(&self) -> &Self::Target {
        &self.app
    }
}
impl DerefMut for WinitApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.app
    }
}

impl winit::application::ApplicationHandler for WinitApp {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.awaiting_destroyed || self.app.world().get_resource::<WinitWindow>().is_some() {
            return;
        }
        let window = event_loop
            .create_window(
                winit::window::WindowAttributes::default()
                    .with_resizable(self.config.resizable)
                    .with_title(self.config.title.clone()),
            )
            .unwrap();
        self.app.world_mut().insert_resource(Window::Window {
            raw_window_handle: window.window_handle().unwrap().as_raw(),
            raw_display_handle: window.display_handle().unwrap().as_raw(),
            physical_size: (window.inner_size().width, window.inner_size().height),
            size_changed: true,
        });
        self.app
            .world_mut()
            .insert_resource(WinitWindow(Arc::new(window)));
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent;
        match event {
            WindowEvent::Resized(physical_size) => {
                if self.awaiting_destroyed {
                    return;
                }
                if physical_size.width == 0 || physical_size.height == 0 {
                    self.app.world_mut().insert_resource(Window::None);
                    self.send(WindowMessage::Suspended);
                    return;
                }
                let (raw_window_handle, raw_display_handle) = {
                    let window = self.app.world().get_resource::<WinitWindow>().unwrap();
                    (
                        window.window_handle().unwrap().as_raw(),
                        window.display_handle().unwrap().as_raw(),
                    )
                };
                self.app.world_mut().insert_resource(Window::Window {
                    raw_window_handle,
                    raw_display_handle,
                    physical_size: (physical_size.width, physical_size.height),
                    size_changed: true,
                });
                self.send(WindowMessage::Resized {
                    width: physical_size.width,
                    height: physical_size.height,
                });
            }
            WindowEvent::CloseRequested
                if !self.awaiting_destroyed => {
                    self.begin_close();
                }
            WindowEvent::Destroyed
                if self.awaiting_destroyed => {
                    event_loop.exit();
                }
            WindowEvent::CursorMoved { position, .. } => {
                let Some(window) = self.app.world().get_resource::<WinitWindow>() else {
                    return;
                };
                let position = position.to_logical(window.scale_factor());
                let position = glam::Vec2::new(position.x, position.y);
                if let Some(last_position) = self.last_position {
                    self.push_input(Input::MouseDelta(position - last_position));
                }
                self.last_position = Some(position);
            }
            WindowEvent::CursorLeft { .. } => {
                self.last_position = None;
            }
            WindowEvent::ModifiersChanged(modifier) => {
                self.modifier_state = modifier.state();
                if let Some(mut log) = self.app.world_mut().get_resource_mut::<InputLog>() {
                    log.set_modifiers(self.modifier_state);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.push_input(Input::KeyEvent {
                    event,
                    modifiers: self.modifier_state,
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.push_input(Input::MouseButton { button, state });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.push_input(Input::MouseWheel(delta));
            }
            _ => {}
        }
    }

    fn suspended(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.awaiting_destroyed {
            return;
        }
        self.app.world_mut().insert_resource(Window::None);
        self.send(WindowMessage::Suspended);
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.awaiting_destroyed {
            return;
        }
        self.app.tick();
        if let Some(window) = self.app.world().get_resource::<WinitWindow>() {
            window.request_redraw();
        }
    }
}
