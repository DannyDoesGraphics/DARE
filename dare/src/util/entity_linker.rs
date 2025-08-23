use bevy_ecs::entity::EntityHashMap;
use bevy_ecs::prelude::*;
use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// Links components from 2 different worlds together
#[derive(Debug)]
pub struct ComponentsLinker {}

enum ComponentsLinkerDelta<T: Component + Clone> {
    Add { entity: Entity, component: T },
    Remove { entity: Entity },
}

impl ComponentsLinker {
    pub fn default<T: Component + Send + Clone>()
    -> (ComponentsLinkerSender<T>, ComponentsLinkerReceiver<T>) {
        let (send, recv) = crossbeam_channel::unbounded::<ComponentsLinkerDelta<T>>();
        (
            ComponentsLinkerSender { send },
            ComponentsLinkerReceiver { recv },
        )
    }
}

#[derive(Debug, Clone)]
pub struct ComponentsLinkerReceiver<T: Component + Clone> {
    recv: crossbeam_channel::Receiver<ComponentsLinkerDelta<T>>,
}

/// Provides entity mappings
#[derive(Debug, Resource)]
struct ComponentsMapping {
    mappings: EntityHashMap<Entity>,
}
impl Deref for ComponentsMapping {
    type Target = EntityHashMap<Entity>;

    fn deref(&self) -> &Self::Target {
        &self.mappings
    }
}
impl DerefMut for ComponentsMapping {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.mappings
    }
}

impl<T: Component + Clone> ComponentsLinkerReceiver<T> {
    pub fn attach_to_world(&self, world: &mut World, schedule: &mut Schedule) {
        let queue = self.recv.clone();
        if world.contains_resource::<ComponentsMapping>() {}
        world.insert_resource(ComponentsMapping {
            mappings: Default::default(),
        });
        // Mapping between send entities -> recv entities
        schedule.add_systems(
            move |mut commands: Commands, mut mappings: ResMut<ComponentsMapping>| {
                while let Ok(delta) = queue.try_recv() {
                    match delta {
                        ComponentsLinkerDelta::Add { entity, component } => {
                            match mappings.get(&entity) {
                                None => {
                                    // Mapping does not exist
                                    // Ensured entity corresponding entity does not exist as well
                                    let recv_entity = commands.spawn(component.clone()).id();
                                    mappings.insert(entity, recv_entity);
                                }
                                Some(recv_entity) => {
                                    // Entity already exists, just insert
                                    commands
                                        .entity(recv_entity.clone())
                                        .insert(component.clone());
                                }
                            }
                        }
                        ComponentsLinkerDelta::Remove { entity } => {
                            if let Some(recv_entity) = mappings.get(&entity) {
                                commands.entity(*recv_entity).remove::<T>();
                            }
                        }
                    }
                }
            },
        );
    }
}

#[derive(Debug, Resource, Clone)]
pub struct ComponentsLinkerSender<T: Component + Clone> {
    send: crossbeam_channel::Sender<ComponentsLinkerDelta<T>>,
}

impl<T: Component + Clone> ComponentsLinkerSender<T> {
    pub fn attach_to_world(&self, send_world: &mut Schedule) {
        let queue = self.send.clone();
        send_world.add_systems(move |query: Query<(Entity, &T), Added<T>>| {
            for (entity, component) in query.iter() {
                queue
                    .send(ComponentsLinkerDelta::Add {
                        entity,
                        component: component.clone(),
                    })
                    .unwrap()
            }
        });
        let queue = self.send.clone();
        send_world.add_systems(move |mut removed: RemovedComponents<T>| {
            for entity in removed.read() {
                queue
                    .send(ComponentsLinkerDelta::Remove { entity })
                    .unwrap()
            }
        });
    }
}
