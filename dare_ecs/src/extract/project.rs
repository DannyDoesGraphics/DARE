use bevy_ecs::query::QueryFilter;
use bevy_ecs::system::SystemState;
use bevy_ecs::{entity::EntityHashMap, prelude::*};
use std::marker::PhantomData;

use crate::{App, Plugin, SubAppLabel};

enum DeltaChange<T: Project> {
    Add(Entity, T),
    Changed(Entity, T),
    ComponentRemove(Entity),
    EntityRemoved(Entity),
}

/// Component projected from world `From` to world `To`.
pub trait Project: Send + Clone + Component {
    type Filter: QueryFilter;
}

/// Defines relationship between entities from `From` to `To`
#[derive(Resource)]
struct ProjectMapping<T: Project> {
    mapping: EntityHashMap<Entity>,
    _marker: PhantomData<T>,
}

/// Projects component T from world `From` to world `To`
pub struct ProjectPlugin<From: SubAppLabel, To: SubAppLabel, T: Project> {
    _marker: PhantomData<(From, To, T)>,
}

impl<From: SubAppLabel, To: SubAppLabel, T: Project> Default for ProjectPlugin<From, To, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<From: SubAppLabel, To: SubAppLabel, T: Project> ProjectPlugin<From, To, T> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<From: SubAppLabel, To: SubAppLabel, T: Project> Plugin for ProjectPlugin<From, To, T> {
    fn build(&self, app: &mut App) {
        app.get_sub_app_mut::<To>()
            .unwrap()
            .world_mut()
            .insert_resource(ProjectMapping::<T> {
                mapping: EntityHashMap::new(),
                _marker: PhantomData,
            });

        app.add_plugin(
            crate::extract::ExtractPlugin::<Vec<DeltaChange<T>>, To, From>::new(
                |world| {
                    let mut deltas: Vec<DeltaChange<T>> = Vec::new();
                    let mut state: SystemState<(
                        Query<(Entity, Ref<T>), T::Filter>,
                        RemovedComponents<T>,
                    )> = SystemState::new(world);

                    let (changes, mut removed) = state.get(world);
                    for entity in removed.read() {
                        if world.get_entity(entity).is_ok() {
                            deltas.push(DeltaChange::ComponentRemove(entity));
                        } else {
                            deltas.push(DeltaChange::EntityRemoved(entity));
                        }
                    }
                    for (entity, component) in &changes {
                        if component.is_added() {
                            deltas.push(DeltaChange::Add(entity, component.clone()));
                        } else if component.is_changed() {
                            deltas.push(DeltaChange::Changed(entity, component.clone()));
                        }
                    }

                    (!deltas.is_empty()).then_some(deltas)
                },
                |world, deltas| {
                    let mut net: EntityHashMap<DeltaChange<T>> = EntityHashMap::new();
                    for snapshot in deltas {
                        for delta in snapshot {
                            let entity = match &delta {
                                DeltaChange::Add(e, _)
                                | DeltaChange::Changed(e, _)
                                | DeltaChange::ComponentRemove(e)
                                | DeltaChange::EntityRemoved(e) => *e,
                            };
                            let merged = match (net.remove(&entity), delta) {
                                (None, d) => Some(d),

                                (Some(DeltaChange::Add(_, _)), DeltaChange::Add(_, v)) => {
                                    Some(DeltaChange::Add(entity, v))
                                }
                                (Some(DeltaChange::Add(_, _)), DeltaChange::Changed(_, v)) => {
                                    Some(DeltaChange::Changed(entity, v))
                                }
                                (Some(DeltaChange::Add(_, _)), DeltaChange::ComponentRemove(_))
                                | (Some(DeltaChange::Add(_, _)), DeltaChange::EntityRemoved(_)) => {
                                    None
                                }

                                (Some(DeltaChange::Changed(_, _)), DeltaChange::Add(_, v)) => {
                                    Some(DeltaChange::Add(entity, v))
                                }
                                (Some(DeltaChange::Changed(_, _)), DeltaChange::Changed(_, v)) => {
                                    Some(DeltaChange::Changed(entity, v))
                                }
                                (
                                    Some(DeltaChange::Changed(_, _)),
                                    DeltaChange::ComponentRemove(_),
                                ) => Some(DeltaChange::ComponentRemove(entity)),
                                (
                                    Some(DeltaChange::Changed(_, _)),
                                    DeltaChange::EntityRemoved(_),
                                ) => Some(DeltaChange::EntityRemoved(entity)),

                                (Some(DeltaChange::ComponentRemove(_)), DeltaChange::Add(_, v)) => {
                                    Some(DeltaChange::Add(entity, v))
                                }
                                (
                                    Some(DeltaChange::ComponentRemove(_)),
                                    DeltaChange::Changed(_, v),
                                ) => Some(DeltaChange::Changed(entity, v)),
                                (
                                    Some(DeltaChange::ComponentRemove(_)),
                                    DeltaChange::ComponentRemove(_),
                                ) => Some(DeltaChange::ComponentRemove(entity)),
                                (
                                    Some(DeltaChange::ComponentRemove(_)),
                                    DeltaChange::EntityRemoved(_),
                                ) => Some(DeltaChange::EntityRemoved(entity)),

                                (Some(DeltaChange::EntityRemoved(_)), DeltaChange::Add(_, v)) => {
                                    Some(DeltaChange::Add(entity, v))
                                }
                                (
                                    Some(DeltaChange::EntityRemoved(_)),
                                    DeltaChange::Changed(_, v),
                                ) => Some(DeltaChange::Changed(entity, v)),
                                (
                                    Some(DeltaChange::EntityRemoved(_)),
                                    DeltaChange::ComponentRemove(_),
                                )
                                | (
                                    Some(DeltaChange::EntityRemoved(_)),
                                    DeltaChange::EntityRemoved(_),
                                ) => Some(DeltaChange::EntityRemoved(entity)),
                            };
                            if let Some(delta) = merged {
                                net.insert(entity, delta);
                            }
                        }
                    }

                    let mut entity_map = std::mem::take(
                        &mut world
                            .get_resource_mut::<ProjectMapping<T>>()
                            .unwrap()
                            .mapping,
                    );

                    {
                        let mut commands = world.commands();
                        for (from, delta) in net {
                            match delta {
                                DeltaChange::Add(_, component)
                                | DeltaChange::Changed(_, component) => {
                                    if let Some(&to) = entity_map.get(&from) {
                                        commands.entity(to).insert(component);
                                    } else {
                                        let to = commands.spawn(component).id();
                                        entity_map.insert(from, to);
                                    }
                                }
                                DeltaChange::ComponentRemove(_) => {
                                    if let Some(&to) = entity_map.get(&from) {
                                        commands.entity(to).remove::<T>();
                                    }
                                }
                                DeltaChange::EntityRemoved(_) => {
                                    if let Some(to) = entity_map.remove(&from) {
                                        commands.entity(to).despawn();
                                    }
                                }
                            }
                        }
                    }
                    world.flush();

                    world
                        .get_resource_mut::<ProjectMapping<T>>()
                        .unwrap()
                        .mapping = entity_map;
                },
            ),
        );
    }
}
