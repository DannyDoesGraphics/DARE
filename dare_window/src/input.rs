use dagal::winit;

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    KeyEvent {
        event: winit::event::KeyEvent,
        modifiers: winit::keyboard::ModifiersState,
    },
    MouseButton {
        button: winit::event::MouseButton,
        state: winit::event::ElementState,
    },
    MouseWheel(winit::event::MouseScrollDelta),
    MouseDelta(glam::Vec2),
}
