use dagal::winit;
use dare_window::Input;

pub fn open_gltf_pressed(input: &Input) -> bool {
    let Input::KeyEvent { event, modifiers } = input else {
        return false;
    };
    event.state.is_pressed()
        && !event.repeat
        && (modifiers.control_key() || modifiers.super_key())
        && event.physical_key == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyO)
}
