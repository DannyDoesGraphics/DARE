use bevy_ecs::prelude as becs;
use std::ops::{Deref, DerefMut};

#[derive(PartialEq, Eq, Hash, Debug, becs::Component, Clone)]
pub struct Name(pub String);

impl Deref for Name {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl DerefMut for Name {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut_str()
    }
}
