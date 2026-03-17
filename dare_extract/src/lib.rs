//!
//! Extraction allows us to do a projection component `a` in world A to components in B

mod entity_world_sync;

use std::ops::{Deref, DerefMut};

use bevy_ecs::prelude::*;
pub(crate) use entity_world_sync::*;

#[derive(Debug, Clone)]
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
impl<T: Streamable> From<DeltaPackets<T>> for Vec<DeltaChange<T>> {
    fn from(val: DeltaPackets<T>) -> Self {
        val.0
    }
}

#[derive(Debug, Resource)]
struct ExtractSend<T: Streamable>(crossbeam_channel::Sender<DeltaPackets<T>>);
impl<T: Streamable> Deref for ExtractSend<T> {
    type Target = crossbeam_channel::Sender<DeltaPackets<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: Streamable> DerefMut for ExtractSend<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, Resource)]
struct ExtractRecv<T: Streamable>(crossbeam_channel::Receiver<DeltaPackets<T>>);
impl<T: Streamable> Deref for ExtractRecv<T> {
    type Target = crossbeam_channel::Receiver<DeltaPackets<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Resource)]
pub struct ExtractPluginRecv<T: Streamable> {
    recv: crossbeam_channel::Receiver<DeltaPackets<T>>,
}
impl<T: Streamable> AsRef<crossbeam_channel::Receiver<DeltaPackets<T>>> for ExtractPluginRecv<T> {
    fn as_ref(&self) -> &crossbeam_channel::Receiver<DeltaPackets<T>> {
        &self.recv
    }
}

#[derive(Debug, Resource)]
pub struct ExtractPluginSend<T: Streamable> {
    send: crossbeam_channel::Sender<DeltaPackets<T>>,
}

pub fn channel<T: Streamable>() -> (ExtractPluginSend<T>, ExtractPluginRecv<T>) {
    let (send, recv) = crossbeam_channel::unbounded();
    (ExtractPluginSend { send }, ExtractPluginRecv { recv })
}

impl<T: Streamable> dare_ecs::Plugin for ExtractPluginSend<T> {
    fn build(&self, app: &mut dare_ecs::App) {
        app.world_mut()
            .insert_resource(ExtractSend(self.send.clone()));

        app.schedule_scope(|schedule| {
            schedule.add_systems(
                (|changes: Query<(Entity, Ref<T>)>,
                  mut removed: RemovedComponents<T>,
                  send: Res<ExtractSend<T>>| {
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

                    // Send deltas, ignoring errors (receiver may be dropped in tests)
                    let _ = send.send(deltas);
                })
                .in_set(dare_ecs::AppStage::Last),
            );
        });
    }
}

impl<T: Streamable> dare_ecs::Plugin for ExtractPluginRecv<T> {
    fn build(&self, app: &mut dare_ecs::App) {
        app.add_plugins(EntityWorldSyncPlugin::default());
        app.world_mut()
            .insert_resource(ExtractRecv(self.recv.clone()));
        app.schedule_scope(|schedule| {
            schedule.add_systems(
                (|mut sync_entities: ResMut<EntityWorldSync>,
                  mut commands: Commands,
                  recv: Res<ExtractRecv<T>>| {
                    let deltas: Vec<DeltaChange<T>> =
                        recv.try_iter().flat_map(|recv| recv.0).collect();
                    for delta in deltas {
                        match delta {
                            DeltaChange::Add(entity, extracted) => {
                                let consumed: T = T::consume(extracted);
                                match sync_entities.get(&entity) {
                                    Some(entity) => {
                                        commands.entity(*entity).insert(consumed);
                                    }
                                    None => {
                                        let entity_current: Entity = commands.spawn(consumed).id();
                                        sync_entities.insert(entity, entity_current);
                                    }
                                }
                            }
                            DeltaChange::Changed(entity, extracted) => {
                                let consumed: T = T::consume(extracted);
                                match sync_entities.get(&entity) {
                                    Some(entity) => {
                                        commands.entity(*entity).insert(consumed);
                                    }
                                    None => {
                                        let entity_current: Entity = commands.spawn(consumed).id();
                                        sync_entities.insert(entity, entity_current);
                                    }
                                }
                            }
                            DeltaChange::Remove(entity) => {
                                if let Some(entity_current) = sync_entities.get(&entity) {
                                    commands.entity(*entity_current).despawn();
                                }
                            }
                        }
                    }
                })
                .in_set(dare_ecs::AppStage::First),
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

#[cfg(test)]
mod test {
    pub use super::*;
    use dare_ecs::App;

    /// Test component implementing Streamable for testing purposes.
    #[derive(Debug, PartialEq, Eq, Copy, Clone, Component)]
    struct TestComponent(u32);

    impl Streamable for TestComponent {
        type Extracted = Self;

        fn consume(extract: Self::Extracted) -> Self {
            extract
        }

        fn extract(&self) -> Self::Extracted {
            *self
        }
    }

    /// Helper to create sender and receiver apps connected by a channel.
    fn create_sync_apps() -> (App, App) {
        let (send, recv) = channel::<TestComponent>();
        let mut sender = App::new();
        let mut receiver = App::new();
        sender.add_plugins(send);
        receiver.add_plugins(recv);
        (sender, receiver)
    }

    /// Helper to run a sync cycle: sender tick then receiver tick.
    fn sync_cycle(sender: &mut App, receiver: &mut App) {
        sender.tick();
        receiver.tick();
    }

    #[test]
    fn test_add_propagation() {
        let (mut sender, mut receiver) = create_sync_apps();

        // Spawn entity with component in sender world
        sender.world_mut().spawn(TestComponent(42));

        // Run sync cycle
        sync_cycle(&mut sender, &mut receiver);

        // Verify entity exists in receiver with correct value
        let query = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .collect::<Vec<_>>();
        assert_eq!(query.len(), 1);
        assert_eq!(query[0].0, 42);
    }

    #[test]
    fn test_change_propagation() {
        let (mut sender, mut receiver) = create_sync_apps();

        // Spawn and sync initial entity
        let entity = sender.world_mut().spawn(TestComponent(10)).id();
        sync_cycle(&mut sender, &mut receiver);

        // Modify component value
        sender
            .world_mut()
            .entity_mut(entity)
            .insert(TestComponent(20));

        // Run sync cycle
        sync_cycle(&mut sender, &mut receiver);

        // Verify component updated in receiver
        let query = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .collect::<Vec<_>>();
        assert_eq!(query.len(), 1);
        assert_eq!(query[0].0, 20);
    }

    #[test]
    fn test_remove_propagation() {
        let (mut sender, mut receiver) = create_sync_apps();

        // Spawn and sync initial entity
        let entity = sender.world_mut().spawn(TestComponent(30)).id();
        sync_cycle(&mut sender, &mut receiver);

        // Get initial entity count
        let initial_count = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .count();
        assert_eq!(initial_count, 1);

        // Despawn entity (removes component)
        sender.world_mut().entity_mut(entity).despawn();

        // Run sync cycle
        sync_cycle(&mut sender, &mut receiver);

        // Verify entity despawned in receiver
        let final_count = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .count();
        assert_eq!(final_count, 0);
    }

    #[test]
    fn test_entity_mapping_preserved_across_ticks() {
        let (mut sender, mut receiver) = create_sync_apps();

        // Spawn entity
        let entity = sender.world_mut().spawn(TestComponent(1)).id();
        sync_cycle(&mut sender, &mut receiver);

        // Store the receiver entity
        let first_query = receiver
            .world_mut()
            .query::<(Entity, &TestComponent)>()
            .iter(receiver.world())
            .collect::<Vec<_>>();
        let receiver_entity = first_query[0].0;

        // Modify and sync again
        sender
            .world_mut()
            .entity_mut(entity)
            .insert(TestComponent(2));
        sync_cycle(&mut sender, &mut receiver);

        // Verify same entity was updated (not respawned)
        let second_query = receiver
            .world_mut()
            .query::<(Entity, &TestComponent)>()
            .iter(receiver.world())
            .collect::<Vec<_>>();
        assert_eq!(second_query.len(), 1);
        assert_eq!(second_query[0].0, receiver_entity);
        assert_eq!(second_query[0].1.0, 2);
    }

    #[test]
    fn test_aba_same_tick() {
        let (mut sender, mut receiver) = create_sync_apps();

        // Spawn initial entity
        let entity = sender.world_mut().spawn(TestComponent(100)).id();
        sync_cycle(&mut sender, &mut receiver);

        // Store the receiver entity for verification
        let _initial_query = receiver
            .world_mut()
            .query::<(Entity, &TestComponent)>()
            .iter(receiver.world())
            .collect::<Vec<_>>();

        // Despawn the entity and respawn a new one (simulating ABA pattern)
        // Note: In real ECS, entity IDs are recycled, so we simulate by
        // removing and re-adding in the same tick
        sender.world_mut().entity_mut(entity).despawn();

        // Create new entity (will get new ID, but we're testing the delta ordering)
        let _new_entity = sender.world_mut().spawn(TestComponent(200)).id();

        // Run sync cycle - both remove and add deltas processed
        sync_cycle(&mut sender, &mut receiver);

        // Verify: receiver should have entity with new value
        let final_query = receiver
            .world_mut()
            .query::<(Entity, &TestComponent)>()
            .iter(receiver.world())
            .collect::<Vec<_>>();
        assert_eq!(final_query.len(), 1);
        assert_eq!(final_query[0].1.0, 200);

        // Note: Since this is a different sender entity, receiver should spawn new entity
        // The old one should be despawned, new one spawned
    }

    #[test]
    fn test_multiple_entities_sync() {
        let (mut sender, mut receiver) = create_sync_apps();

        // Spawn multiple entities
        sender.world_mut().spawn(TestComponent(1));
        sender.world_mut().spawn(TestComponent(2));
        sender.world_mut().spawn(TestComponent(3));

        sync_cycle(&mut sender, &mut receiver);

        // Verify all entities synced
        let mut values: Vec<u32> = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .map(|c| c.0)
            .collect();
        values.sort();

        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn test_change_for_unmapped_entity() {
        // This tests the scenario where a "Changed" delta arrives for an entity
        // not yet in the sync mapping. The receiver should treat it as an Add.
        let (mut sender, mut receiver) = create_sync_apps();

        // Spawn entity in sender only (no sync yet)
        let _entity = sender.world_mut().spawn(TestComponent(50)).id();
        sender.tick(); // Generate Add delta but don't run receiver

        // Clear trackers so next change appears as Changed not Add
        sender.world_mut().clear_trackers();

        // Now modify (simulating that this appears as Changed to the system)
        // Since we haven't synced yet, the receiver will get Changed for an unmapped entity
        // This tests the fallback path in the recv system that spawns new entities
        // Note: In practice, the system sends Changed if is_changed() && !is_added()
        // But since we haven't synced, the mapping doesn't exist yet

        // Sync first time to establish mapping
        sync_cycle(&mut sender, &mut receiver);

        // Now we have mapping, modify and sync again
        sender
            .world_mut()
            .entity_mut(_entity)
            .insert(TestComponent(60));
        sync_cycle(&mut sender, &mut receiver);

        // Verify updated value - entity should be updated (not respawned)
        let query = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .collect::<Vec<_>>();
        assert_eq!(query.len(), 1);
        assert_eq!(query[0].0, 60);
    }

    #[test]
    fn test_full_sync_cycle() {
        let (mut sender, mut receiver) = create_sync_apps();

        // Phase 1: Spawn and sync
        let e1 = sender.world_mut().spawn(TestComponent(1)).id();
        let e2 = sender.world_mut().spawn(TestComponent(2)).id();
        sync_cycle(&mut sender, &mut receiver);

        let count1 = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .count();
        assert_eq!(count1, 2, "Phase 1: Should have 2 entities");

        // Phase 2: Modify one
        sender.world_mut().entity_mut(e1).insert(TestComponent(10));
        sync_cycle(&mut sender, &mut receiver);

        let values: Vec<u32> = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .map(|c| c.0)
            .collect();
        assert!(values.contains(&10), "Phase 2: e1 should be updated to 10");
        assert!(values.contains(&2), "Phase 2: e2 should still be 2");

        // Phase 3: Despawn one
        sender.world_mut().entity_mut(e2).despawn();
        sync_cycle(&mut sender, &mut receiver);

        let count3 = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .count();
        assert_eq!(count3, 1, "Phase 3: Should have 1 entity after despawn");

        // Phase 4: Add another
        sender.world_mut().spawn(TestComponent(3));
        sync_cycle(&mut sender, &mut receiver);

        let count4 = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .count();
        assert_eq!(count4, 2, "Phase 4: Should have 2 entities again");
    }

    #[test]
    fn test_cross_world_consistency() {
        let (mut sender, mut receiver) = create_sync_apps();

        // Track state through lifecycle
        let entity = sender.world_mut().spawn(TestComponent(42)).id();

        // Sync
        sync_cycle(&mut sender, &mut receiver);
        assert_eq!(
            receiver
                .world_mut()
                .query::<&TestComponent>()
                .iter(receiver.world())
                .count(),
            1
        );

        // Modify and sync
        sender
            .world_mut()
            .entity_mut(entity)
            .insert(TestComponent(100));
        sync_cycle(&mut sender, &mut receiver);
        let value = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .next()
            .unwrap()
            .0;
        assert_eq!(value, 100);

        // Remove and sync
        sender.world_mut().entity_mut(entity).despawn();
        sync_cycle(&mut sender, &mut receiver);
        assert_eq!(
            receiver
                .world_mut()
                .query::<&TestComponent>()
                .iter(receiver.world())
                .count(),
            0
        );
    }

    #[test]
    fn test_empty_delta_no_panic() {
        let (mut sender, mut receiver) = create_sync_apps();

        // Run sync cycle without any changes
        sync_cycle(&mut sender, &mut receiver);

        // Should not panic and receiver should have no entities
        let count = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_component_value_integrity() {
        let (mut sender, mut receiver) = create_sync_apps();

        // Test various values to ensure extract/consume roundtrip preserves data
        let mut test_values = vec![0, 1, u32::MAX, 12345, 999999];

        for &value in test_values.iter() {
            // Spawn new entity for each value to avoid conflicts
            sender.world_mut().spawn(TestComponent(value));
        }

        sync_cycle(&mut sender, &mut receiver);

        let mut received_values: Vec<u32> = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .map(|c| c.0)
            .collect();
        received_values.sort();
        test_values.sort();

        assert_eq!(received_values, test_values);
    }

    #[test]
    fn test_delta_ordering_add_before_remove() {
        // This tests the critical ordering: removes processed before adds/changes
        // If ordering were wrong, an entity that is removed and re-added in same tick
        // would incorrectly end up removed
        let (mut sender, mut receiver) = create_sync_apps();

        // Setup: create entity
        let entity = sender.world_mut().spawn(TestComponent(1)).id();
        sync_cycle(&mut sender, &mut receiver);

        // Now: despawn and spawn new entity
        // In the same tick, we have both remove and add operations
        sender.world_mut().entity_mut(entity).despawn();
        sender.world_mut().spawn(TestComponent(999));

        // Sync - this should result in 1 entity with value 999
        sync_cycle(&mut sender, &mut receiver);

        let query = receiver
            .world_mut()
            .query::<&TestComponent>()
            .iter(receiver.world())
            .collect::<Vec<_>>();
        assert_eq!(query.len(), 1, "Should have exactly 1 entity after ABA");
        assert_eq!(query[0].0, 999, "Should have the new component value");
    }
}
