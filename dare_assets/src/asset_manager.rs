use bevy_ecs::prelude::*;
use futures::SinkExt;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{GeometryDescription, GeometryDescriptionHandle, MeshAsset, MeshHandle};

/// Commands to send to the render side of the geometry manager
#[derive(Debug, Message)]
pub enum RenderAssetCommand {
    CreateGeometry {
        handle: GeometryDescriptionHandle,
        description: GeometryDescription,
        runtime: Arc<crate::GeometryRuntime>,
    },
    DestroyGeometry {
        handle: GeometryDescriptionHandle,
    },
    CreateMesh {
        handle: MeshHandle,
        mesh: MeshAsset,
    },
    DestroyMesh {
        handle: MeshHandle,
    },
}

/// Asset manager is responsible for handling high-level asset operations.
///
/// # Send/Recieve functionality
/// The asset manager has send and recieve functionality. It works by returning a send/recieve pair upon the creation of a new asset managers. Each pair must
///
/// ## Communication
/// Changes are communicated via [`RenderAssetCommand`] which effectively deltas via a crossbeam channel.
#[derive(Debug, Resource, Default)]
pub struct AssetManager {
    ttl: u16,
    geometry_descriptions:
        dare_containers::slot_map::SlotMap<GeometryDescription, GeometryDescriptionHandle>,
    pub geometry_runtime:
        HashMap<GeometryDescriptionHandle, std::sync::Arc<crate::geometry::GeometryRuntime>>,
    pub mesh_store: dare_containers::slot_map::SlotMap<MeshAsset, MeshHandle>,
    buffer: Vec<RenderAssetCommand>,
    send: Option<crossbeam_channel::Sender<RenderAssetCommand>>,
    recv: Option<crossbeam_channel::Receiver<RenderAssetCommand>>,
}
unsafe impl Send for AssetManager {}

impl AssetManager {
    /// Returns a pair of asset manager where: (send,recv)
    pub fn new(ttl: u16) -> (Self, Self) {
        let (send, recv) = crossbeam_channel::unbounded::<RenderAssetCommand>();
        (
            Self {
                ttl,
                send: Some(send),
                ..Default::default()
            },
            Self {
                ttl,
                recv: Some(recv),
                ..Default::default()
            },
        )
    }

    /// Create an entirely new geometry and ensures geometries are backed by a [`crate::geometry::GeometryRuntime`]
    pub fn create_geometry(
        &mut self,
        geometry: crate::GeometryDescription,
    ) -> crate::GeometryDescriptionHandle {
        let (handle, runtime) = self.create_geometry_with_runtime(geometry.clone(), None);
        self.buffer.push(RenderAssetCommand::CreateGeometry {
            handle,
            description: geometry,
            runtime: runtime.clone(),
        });
        handle
    }

    /// If runtime is [`None`], a new one will be made.
    fn create_geometry_with_runtime(
        &mut self,
        description: crate::GeometryDescription,
        runtime: Option<Arc<crate::GeometryRuntime>>,
    ) -> (
        crate::GeometryDescriptionHandle,
        Arc<crate::GeometryRuntime>,
    ) {
        let handle = self.geometry_descriptions.insert(description);
        let runtime = runtime.unwrap_or(Arc::new(crate::geometry::GeometryRuntime {
            ttl: std::sync::atomic::AtomicU16::from(self.ttl),
            ..Default::default()
        }));

        assert!(
            self.geometry_runtime
                .insert(handle, runtime.clone())
                .is_none(),
            "All runtimes should be None"
        );
        (handle, runtime)
    }

    /// Remove a geometry, return [`None`] if removing a non-existent geometry
    pub fn remove_geometry(
        &mut self,
        handle: crate::GeometryDescriptionHandle,
    ) -> Option<crate::GeometryDescription> {
        self.geometry_descriptions
            .remove(handle)
            .inspect(|_| {
                self.geometry_runtime.remove(&handle);
                self.buffer.push(RenderAssetCommand::DestroyGeometry {
                    handle: handle.clone(),
                })
            })
            .ok()
    }

    /// Tick is an operation that ensures that
    pub fn tick(&mut self) {
        if let Some(send) = self.send.as_ref() {
            self.buffer.drain(..).for_each(|command| {
                send.send(command).unwrap();
            });
        } else if let Some(recv) = self.recv.as_ref() {
            let commands: Vec<RenderAssetCommand> =
                recv.try_iter().collect::<Vec<RenderAssetCommand>>();
            for command in commands {
                match command {
                    RenderAssetCommand::CreateGeometry {
                        handle: _handle,
                        description,
                        runtime,
                    } => {
                        // in theory, we don't need a handle given we keep it in lock-step
                        self.create_geometry_with_runtime(description, Some(runtime));
                    }
                    RenderAssetCommand::DestroyGeometry { handle } => {
                        self.geometry_descriptions.remove(handle).ok();
                        self.geometry_runtime.remove(&handle);
                    }
                    RenderAssetCommand::CreateMesh { handle: _, mesh } => {
                        self.mesh_store.insert(mesh);
                    }
                    RenderAssetCommand::DestroyMesh { handle } => {
                        self.mesh_store.remove(handle).ok();
                    }
                }
            }
        }
    }

    #[cfg(test)]
    fn geometry_count(&self) -> usize {
        self.geometry_descriptions.iter().count()
    }

    #[cfg(test)]
    fn runtime_count(&self) -> usize {
        self.geometry_runtime.len()
    }

    #[cfg(test)]
    fn contains_geometry(&self, handle: GeometryDescriptionHandle) -> bool {
        self.geometry_descriptions.get(handle).is_some()
    }

    #[cfg(test)]
    fn contains_runtime(&self, handle: GeometryDescriptionHandle) -> bool {
        self.geometry_runtime.contains_key(&handle)
    }

    #[cfg(test)]
    fn get_geometry(&self, handle: GeometryDescriptionHandle) -> Option<&GeometryDescription> {
        self.geometry_descriptions.get(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataLocation, Format};

    fn make_test_geometry() -> GeometryDescription {
        GeometryDescription {
            location: DataLocation::Blob(std::sync::Arc::from(&[0u8; 4] as &[u8])),
            format: Format::F32x3,
            offset: 0,
            stride: None,
            count: 3,
        }
    }

    #[test]
    fn new_creates_send_recv_pair() {
        let (sender, receiver) = AssetManager::new(100);
        assert!(sender.send.is_some());
        assert!(sender.recv.is_none());
        assert!(receiver.recv.is_some());
        assert!(receiver.send.is_none());
    }

    #[test]
    fn create_geometry_adds_to_sender() {
        let (mut sender, _receiver) = AssetManager::new(100);
        let geometry = make_test_geometry();

        let handle = sender.create_geometry(geometry.clone());

        assert!(sender.contains_geometry(handle));
        assert!(sender.contains_runtime(handle));
        assert_eq!(sender.geometry_count(), 1);
        assert_eq!(sender.runtime_count(), 1);
        assert_eq!(sender.get_geometry(handle), Some(&geometry));
    }

    #[test]
    fn tick_propagates_create_to_receiver() {
        let (mut sender, mut receiver) = AssetManager::new(100);
        let geometry = make_test_geometry();

        let handle = sender.create_geometry(geometry.clone());
        sender.tick();
        receiver.tick();

        assert!(receiver.contains_geometry(handle));
        assert!(receiver.contains_runtime(handle));
        assert_eq!(receiver.geometry_count(), 1);
        assert_eq!(receiver.runtime_count(), 1);
        assert_eq!(receiver.get_geometry(handle), Some(&geometry));
    }

    #[test]
    fn handles_are_identical_across_managers() {
        let (mut sender, mut receiver) = AssetManager::new(100);
        let geometry = make_test_geometry();

        let handle = sender.create_geometry(geometry);
        sender.tick();
        receiver.tick();

        assert!(sender.contains_geometry(handle));
        assert!(receiver.contains_geometry(handle));
    }

    #[test]
    fn remove_geometry_removes_from_sender() {
        let (mut sender, _receiver) = AssetManager::new(100);
        let geometry = make_test_geometry();

        let handle = sender.create_geometry(geometry.clone());
        assert!(sender.contains_geometry(handle));

        let removed = sender.remove_geometry(handle);
        assert_eq!(removed, Some(geometry));
        assert!(!sender.contains_geometry(handle));
        assert!(!sender.contains_runtime(handle));
        assert_eq!(sender.geometry_count(), 0);
        assert_eq!(sender.runtime_count(), 0);
    }

    #[test]
    fn tick_propagates_remove_to_receiver() {
        let (mut sender, mut receiver) = AssetManager::new(100);
        let geometry = make_test_geometry();

        let handle = sender.create_geometry(geometry);
        sender.tick();
        receiver.tick();
        assert!(receiver.contains_geometry(handle));

        sender.remove_geometry(handle);
        sender.tick();
        receiver.tick();

        assert!(!receiver.contains_geometry(handle));
        assert!(!receiver.contains_runtime(handle));
        assert_eq!(receiver.geometry_count(), 0);
        assert_eq!(receiver.runtime_count(), 0);
    }

    #[test]
    fn remove_nonexistent_geometry_returns_none() {
        let (mut sender, _receiver) = AssetManager::new(100);
        let fake_handle = GeometryDescriptionHandle::default();

        let result = sender.remove_geometry(fake_handle);
        assert!(result.is_none());
    }

    #[test]
    fn handle_invalid_after_removal() {
        let (mut sender, _receiver) = AssetManager::new(100);
        let geometry = make_test_geometry();

        let handle = sender.create_geometry(geometry);
        sender.remove_geometry(handle);

        // Slot map should not contain the handle after removal
        // Generation counter should have incremented, making old handle invalid
        assert!(!sender.contains_geometry(handle));
    }

    #[test]
    fn multiple_geometries_sync_correctly() {
        let (mut sender, mut receiver) = AssetManager::new(100);

        let geometry1 = make_test_geometry();
        let mut geometry2 = make_test_geometry();
        geometry2.format = Format::F32x2;

        let handle1 = sender.create_geometry(geometry1.clone());
        let handle2 = sender.create_geometry(geometry2.clone());

        sender.tick();
        receiver.tick();

        // Both managers should have both geometries
        assert_eq!(sender.geometry_count(), 2);
        assert_eq!(receiver.geometry_count(), 2);

        // Verify handles map to correct geometries
        assert_eq!(sender.get_geometry(handle1), Some(&geometry1));
        assert_eq!(receiver.get_geometry(handle1), Some(&geometry1));
        assert_eq!(sender.get_geometry(handle2), Some(&geometry2));
        assert_eq!(receiver.get_geometry(handle2), Some(&geometry2));

        // Runtimes exist for both
        assert!(sender.contains_runtime(handle1));
        assert!(sender.contains_runtime(handle2));
        assert!(receiver.contains_runtime(handle1));
        assert!(receiver.contains_runtime(handle2));
    }

    #[test]
    fn partial_removal_syncs_correctly() {
        let (mut sender, mut receiver) = AssetManager::new(100);

        let geometry1 = make_test_geometry();
        let mut geometry2 = make_test_geometry();
        geometry2.format = Format::F32x2;

        let handle1 = sender.create_geometry(geometry1);
        let handle2 = sender.create_geometry(geometry2);

        sender.tick();
        receiver.tick();

        // Remove only handle1
        sender.remove_geometry(handle1);
        sender.tick();
        receiver.tick();

        // handle1 gone, handle2 remains
        assert!(!sender.contains_geometry(handle1));
        assert!(!receiver.contains_geometry(handle1));
        assert!(sender.contains_geometry(handle2));
        assert!(receiver.contains_geometry(handle2));

        assert_eq!(sender.geometry_count(), 1);
        assert_eq!(receiver.geometry_count(), 1);
    }

    #[test]
    fn operations_before_tick_do_not_affect_receiver() {
        let (mut sender, mut receiver) = AssetManager::new(100);
        let geometry = make_test_geometry();

        let handle = sender.create_geometry(geometry);

        // Receiver hasn't ticked yet, should be empty
        assert!(!receiver.contains_geometry(handle));
        assert_eq!(receiver.geometry_count(), 0);

        // Even after sender ticks, receiver needs to tick too
        sender.tick();
        assert!(!receiver.contains_geometry(handle));

        receiver.tick();
        assert!(receiver.contains_geometry(handle));
    }

    #[test]
    fn ttl_is_propagated_to_runtime() {
        let ttl = 500u16;
        let (mut sender, mut receiver) = AssetManager::new(ttl);
        let geometry = make_test_geometry();

        let handle = sender.create_geometry(geometry);
        sender.tick();
        receiver.tick();

        // Both managers should have the same ttl in their runtimes
        // Access the runtime and check ttl
        let sender_runtime = sender.geometry_runtime.get(&handle).unwrap();
        let receiver_runtime = receiver.geometry_runtime.get(&handle).unwrap();

        use std::sync::atomic::Ordering;
        assert_eq!(sender_runtime.ttl.load(Ordering::SeqCst), ttl);
        assert_eq!(receiver_runtime.ttl.load(Ordering::SeqCst), ttl);
    }

    #[test]
    fn interleaved_add_remove_sequences() {
        let (mut sender, mut receiver) = AssetManager::new(100);

        // Add 3 geometries
        let geometry1 = make_test_geometry();
        let mut geometry2 = make_test_geometry();
        geometry2.format = Format::F32x2;
        let mut geometry3 = make_test_geometry();
        geometry3.format = Format::F64x3;

        let handle1 = sender.create_geometry(geometry1.clone());
        sender.tick();
        receiver.tick();

        // Add one more, remove one
        let handle2 = sender.create_geometry(geometry2.clone());
        sender.remove_geometry(handle1);
        sender.tick();
        receiver.tick();

        // handle1 gone, handle2 present
        assert!(!sender.contains_geometry(handle1));
        assert!(!receiver.contains_geometry(handle1));
        assert!(sender.contains_geometry(handle2));
        assert!(receiver.contains_geometry(handle2));
        assert_eq!(sender.geometry_count(), 1);
        assert_eq!(receiver.geometry_count(), 1);

        // Add third, remove second, add another
        let handle3 = sender.create_geometry(geometry3.clone());
        sender.remove_geometry(handle2);
        let handle4 = sender.create_geometry(geometry1.clone()); // reuse geometry1 data
        sender.tick();
        receiver.tick();

        // Only handle3 and handle4 should remain
        assert!(!sender.contains_geometry(handle1));
        assert!(!sender.contains_geometry(handle2));
        assert!(sender.contains_geometry(handle3));
        assert!(sender.contains_geometry(handle4));
        assert!(receiver.contains_geometry(handle3));
        assert!(receiver.contains_geometry(handle4));
        assert_eq!(sender.geometry_count(), 2);
        assert_eq!(receiver.geometry_count(), 2);

        // Verify contents match
        assert_eq!(receiver.get_geometry(handle3), Some(&geometry3));
        assert_eq!(receiver.get_geometry(handle4), Some(&geometry1));

        // Final cleanup - remove all
        sender.remove_geometry(handle3);
        sender.remove_geometry(handle4);
        sender.tick();
        receiver.tick();

        assert_eq!(sender.geometry_count(), 0);
        assert_eq!(receiver.geometry_count(), 0);
    }
}
