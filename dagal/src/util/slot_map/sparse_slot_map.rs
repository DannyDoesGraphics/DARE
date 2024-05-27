use anyhow::Result;

use crate::util::slot_map::Slot;

#[derive(Debug, Copy, Clone, Default)]
pub struct SlotEntry<T> {
	data: Option<T>,
	slot: Slot<T>,
}

impl<T> PartialEq for SlotEntry<T> {
	fn eq(&self, other: &Self) -> bool {
		self.slot == other.slot
	}
}

/// tl;dr Works much more similar to a [`FreeList`](crate::util::FreeList) with generation counters.
///
/// A SparseSlotMap is a slot map where it does not attempt to dense pack all the data together.
/// When data is deleted, it leaves a gap in the vector and notes that it is free similar to a FreeList.
/// This means we can sacrifice the indices vector and have direct handle mappings to the data in the
/// data vector.
///
/// # Performance characteristics
/// O(1) insertions/deletion
///
/// 1 level of indirection due to direct handle mappings to the underlying data's location
///
/// Faster deletion time as no data swaps must occur
#[derive(Debug, Default)]
pub struct SparseSlotMap<T> {
	/// Store the data right next to it's handle
	data: Vec<SlotEntry<T>>,
	/// List of freed slots
	free_list: Vec<usize>,
}

impl<T> SparseSlotMap<T> {
	pub fn new(capacity: usize) -> Self {
		Self {
			data: Vec::with_capacity(capacity),
			free_list: Vec::new(),
		}
	}

	/// Insert an element into a sparse slot map
	pub fn insert(&mut self, data: T) -> Slot<T> {
		let next_free_index = if self.free_list.is_empty() {
			self.data.push(SlotEntry {
				data: None,
				slot: Slot::new(self.data.len() as u64, None),
			});
			self.free_list.push(self.data.len() - 1);
			self.data.len() - 1
		} else {
			self.free_list.pop().unwrap()
		};
		let slot = self.data.get_mut(next_free_index).unwrap();
		slot.data = Some(data);

		slot.slot.clone()
	}

	/// Remove an element from a SparseSlotMap by slot
	pub fn remove(&mut self, slot: Slot<T>) -> Result<T> {
		if !self.is_valid_slot(&slot) {
			return Err(anyhow::Error::from(crate::DagalError::InvalidSlotMapSlot))
		}
		let slot_union = self.data.get_mut(slot.id as usize).unwrap();
		slot_union.slot.generation += 1; // invalidate
		Ok(slot_union.data.take().unwrap())
	}

	/// Checks if a given slot is valid in the SparseSlotMap
	pub fn is_valid_slot(&self, slot: &Slot<T>) -> bool {
		return self.data.get(slot.id as usize).map(|slot_union| {
			*slot == slot_union.slot && slot_union.data.is_some()
		}).unwrap_or(false)
	}
}