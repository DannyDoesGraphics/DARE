use bevy_ecs::prelude::*;
use std::collections::HashMap;

/// Tracks if an asset has been used before. If so, marked and marked for clean up
#[derive(Debug, Resource)]
pub struct UseMap {
    map: HashMap<u64, bool>,
}

fn erased_hash<A: dare_assets::Asset>(handle: &dare_assets::AssetHandle<A>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::hash::DefaultHasher::default();
    handle.hash(&mut hash);
    std::any::TypeId::of::<A>().hash(&mut hash);
    hash.finish()
}

impl UseMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Mark a resource as accessed. Returns if already accessed prior
    pub fn mark_used<A: dare_assets::Asset>(
        &mut self,
        handle: &dare_assets::AssetHandle<A>,
    ) -> bool {
        self.map.insert(erased_hash(handle), true).unwrap_or(false)
    }

    /// Check if a handle has been accessed prior thus is in use
    pub fn been_used<A: dare_assets::Asset>(&self, handle: &dare_assets::AssetHandle<A>) -> bool {
        self.map
            .get(&erased_hash(handle))
            .map(|v| *v)
            .unwrap_or(false)
    }

    /// Used to clear all usages
    pub fn erase(&mut self) {
        self.map.clear();
    }
}
