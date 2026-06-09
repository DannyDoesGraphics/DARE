use bevy_ecs::prelude::*;

/// Winit → ECS bridge. Written from the event loop, read in scheduled systems.
#[derive(Debug, Clone, Message)]
pub enum WindowMessage {
    CloseRequested,
    Resized { width: u32, height: u32 },
    Suspended,
}
