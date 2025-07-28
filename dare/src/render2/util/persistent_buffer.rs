use super::growable_buffer::GrowableBuffer;
use dagal::allocators::Allocator;
use bevy_ecs::prelude::*;
use dare_containers::prelude as containers;
use dare_containers::traits::Container;
use dare_containers::slot::{DefaultSlot, Slot};
use std::collections::{HashMap, BTreeMap};
use crate::prelude as dare;

/// Represents the delta of changes to a persistent buffer
/// where a unique identifier is passed to identify the change
#[derive(Debug)]
pub enum PersistentDelta<T> {
    Added(u64, T),
    Updated(u64, T),
    Removed(u64)
}

impl<T: Clone> Clone for PersistentDelta<T> {
    fn clone(&self) -> Self {
        match self {
            PersistentDelta::Added(id, value) => PersistentDelta::Added(*id, value.clone()),
            PersistentDelta::Updated(id, value) => PersistentDelta::Updated(*id, value.clone()),
            PersistentDelta::Removed(id) => PersistentDelta::Removed(*id),
        }
    }
}

/// Responsible for handling fine additions, removals, and updates to buffers
/// Uses a free list to manage allocations within the GPU buffer
#[derive(Debug, Resource)]
pub struct PersistentBuffer<A: Allocator + 'static, T: 'static> {
    growable_buffer: GrowableBuffer<A>,
    update_queue: Vec<PersistentDelta<T>>,
    /// Free list to track available slots in the buffer
    free_list: containers::FreeList<()>,
    /// Maps external IDs to internal slots for efficient lookups
    id_to_slot: HashMap<u64, DefaultSlot<()>>,
    /// Size of each element in bytes
    element_size: usize,
}

impl<A: Allocator + 'static, T: 'static> PersistentBuffer<A, T> {
    pub fn new(growable_buffer: GrowableBuffer<A>) -> Self {
        Self {
            growable_buffer,
            update_queue: Vec::new(),
            free_list: containers::FreeList::new(),
            id_to_slot: HashMap::new(),
            element_size: std::mem::size_of::<T>(),
        }
    }

    pub fn submit_queue(&mut self, deltas: Vec<PersistentDelta<T>>) {
        self.update_queue.extend(deltas);
    }

    pub async fn flush_queue(&mut self, immediate_submit: &dare::render::util::ImmediateSubmit) -> anyhow::Result<()> {
        if self.update_queue.is_empty() {
            return Ok(());
        }
        
        // sort to process removals first, then updates, then additions
        self.update_queue.sort_by_key(|delta| match delta {
            PersistentDelta::Removed(_) => 0, // handle removals first
            PersistentDelta::Updated(_, _) => 1, // handle updates next
            PersistentDelta::Added(_, _) => 2, // handle additions last
        });

        let deltas = std::mem::take(&mut self.update_queue);
        
        // Calculate total required capacity
        let mut additional_slots_needed = 0;
        for delta in &deltas {
            match delta {
                PersistentDelta::Added(_, _) => additional_slots_needed += 1,
                _ => {}
            }
        }
        
        // Ensure we have enough capacity in the buffer
        let current_total_slots = self.free_list.total_data_len();
        let required_total_slots = current_total_slots + additional_slots_needed;
        let required_size = required_total_slots * self.element_size;
        self.growable_buffer.reserve(immediate_submit, required_size as u64).await?;
        
        // Use BTreeMap to maintain sorted order by offset for optimal batching
        let mut dirty_writes: BTreeMap<u64, T> = BTreeMap::new();
        
        // Process deltas and collect dirty writes
        for delta in deltas {
            match delta {
                PersistentDelta::Added(id, value) => {
                    let slot = self.free_list.insert(());
                    let offset = slot.id() as u64 * self.element_size as u64;
                    self.id_to_slot.insert(id, slot);
                    dirty_writes.insert(offset, value);
                }
                PersistentDelta::Updated(id, value) => {
                    if let Some(slot) = self.id_to_slot.get(&id) {
                        let offset = slot.id() as u64 * self.element_size as u64;
                        dirty_writes.insert(offset, value);
                    }
                }
                PersistentDelta::Removed(id) => {
                    if let Some(slot) = self.id_to_slot.remove(&id) {
                        let offset = slot.id() as u64 * self.element_size as u64;
                        let _ = self.free_list.remove(slot);
                        // Remove from dirty writes if it was pending
                        dirty_writes.remove(&offset);
                    }
                }
            }
        }
        
        // Batch upload all dirty writes in optimal ranges
        if !dirty_writes.is_empty() {
            self.batch_upload_dirty(immediate_submit, dirty_writes).await?;
        }
        
        Ok(())
    }

    /// Optimized batch upload using BTreeMap for automatic sorting and optimal range detection
    async fn batch_upload_dirty(&mut self, immediate_submit: &dare::render::util::ImmediateSubmit, dirty_writes: BTreeMap<u64, T>) -> anyhow::Result<()> {
        if dirty_writes.is_empty() {
            return Ok(());
        }
        
        let mut writes_iter = dirty_writes.into_iter().peekable();
        
        while let Some((start_offset, first_value)) = writes_iter.next() {
            let mut range_data = vec![first_value];
            let mut current_offset = start_offset;
            
            // Collect all contiguous writes
            while let Some(&(next_offset, _)) = writes_iter.peek() {
                if next_offset == current_offset + self.element_size as u64 {
                    let (_, next_value) = writes_iter.next().unwrap();
                    range_data.push(next_value);
                    current_offset = next_offset;
                } else {
                    break;
                }
            }
            
            // Upload this contiguous range as a single GPU operation
            self.growable_buffer
                .upload_to_buffer_at_offset(immediate_submit, start_offset, &range_data)
                .await?;
        }
        
        Ok(())
    }

    /// Get the underlying GPU buffer
    pub fn get_buffer(&self) -> &GrowableBuffer<A> {
        &self.growable_buffer
    }
}
