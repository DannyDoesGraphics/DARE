use bevy_ecs::prelude as becs;
use std::ops::{Deref, DerefMut};

/// Wraps around [`bevy_ecs::world`]
#[derive(Debug)]
#[repr(transparent)]
pub struct World(pub bevy_ecs::world::World);

impl World {
    pub fn new() -> Self {
        Self(bevy_ecs::world::World::new())
    }

    pub fn add_event<T: Send + 'static>(&mut self) -> super::event::EventSender<T> {
        let (send, recv) = crossbeam_channel::unbounded::<T>();
        let send = super::event::EventSender::new(send);
        self.insert_resource(send.clone());
        self.insert_resource(super::event::EventReceiver::new(recv));
        send
    }
}

impl Deref for World {
    type Target = becs::World;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for World {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
