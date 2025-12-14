#[derive(bevy_ecs::resource::Resource, Debug)]
pub struct Timer {
    pub last_recorded: Option<std::time::Instant>,
}
