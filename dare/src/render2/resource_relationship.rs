use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use std::collections::HashMap;

/// A mapping from the engine world entity id to the render world entity id
#[derive(Debug, becs::Resource, Default)]
pub struct Meshes(pub(crate) HashMap<becs::Entity, becs::Entity>);
