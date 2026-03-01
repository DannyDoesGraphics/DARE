//!
//! Extraction allows us to do a projection component a in world A to components in B
//!
//! # Snapshot
//! Snapshots refer to sending every change from world A to B all at once
//!
//! # Delta
//! Refers to streaming a set of delta changes from world A to B over time

use std::marker::PhantomData;

use bevy_ecs::{entity::EntityHashSet, prelude::*};

#[derive(Debug)]
enum DeltaChange<T: Streamable> {
    Add(Entity, T::Extracted),
    Remove(Entity),
    Changed(Entity, T::Extracted),
}

/// Grouping multiple deltas in one packet so we don't spam the channel
#[derive(Debug)]
struct DeltaPackets<T: Streamable>(Vec<DeltaChange<T>>);
impl<T: Streamable> std::ops::Deref for DeltaPackets<T> {
    type Target = Vec<DeltaChange<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Streamable> std::ops::DerefMut for DeltaPackets<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}



#[derive(Debug, Resource)]
struct ExtractRecvState<T: Streamable> {
    recv: crossbeam_channel::Receiver<DeltaPackets<T>>,
}

#[derive(Debug, Resource)]
struct ExtractSendState<T: Streamable> {
    send: crossbeam_channel::Sender<DeltaPackets<T>>,
}

#[derive(Debug, Resource)]
struct ExtractResource<T: Streamable>(dare_util::Either<crossbeam_channel::Sender<DeltaPackets<T>>, crossbeam_channel::Receiver<DeltaPackets<T>>>);
impl<T: Streamable> std::ops::Deref for ExtractResource<T> {
    type Target = dare_util::Either<crossbeam_channel::Sender<DeltaPackets<T>>, crossbeam_channel::Receiver<DeltaPackets<T>>>;
    
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: Streamable> std::ops::DerefMut for ExtractResource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}


#[derive(Debug)]
pub struct ExtractPlugin<T: Component> {
    _marker: PhantomData<T>,
}

impl<T: Streamable> dare_ecs::Plugin for ExtractPlugin<T> {
    fn build(&self, app: &mut dare_ecs::App) {
        // Schedule extract
        app.schedule_scope(|schedule| {
            schedule.add_systems(
                |changes: Query<(Entity, Ref<T>)>, mut removed: RemovedComponents<T>, extract_resource: Res<ExtractResource<T>> | {
                    
                    let mut deltas: DeltaPackets<T> = DeltaPackets(Vec::new());
                    // Order here ensures ABA is preversed!
                    for entity in removed.read() {
                        deltas.push(DeltaChange::Remove(entity));
                    }
                    for (entity, component) in changes {
                        if component.is_added() {
                            deltas.push(DeltaChange::Add(entity, component.extract()));
                        } else if component.is_changed() {
                            deltas.push(DeltaChange::Changed(entity, component.extract()));
                        }
                    }
                },
            );
        });
    }
}

/// Implies a component can be streamed
pub trait Streamable: Component {
    type Extracted: Send + std::fmt::Debug;

    /// Determines if components should be sent over using a Snapshot vs Delta format
    fn should_delta() -> bool {
        true
    }

    /// Transform component to be sent between worlds for projection
    fn extract(&self) -> Self::Extracted;

    fn consume(extract: Self::Extracted) -> Self;
}
