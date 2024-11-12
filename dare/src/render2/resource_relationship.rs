use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use std::collections::HashMap;

#[derive(Debug, becs::Resource, Default)]
pub struct Surfaces(pub(crate) HashMap<becs::Entity, dare::engine::components::Surface>);
