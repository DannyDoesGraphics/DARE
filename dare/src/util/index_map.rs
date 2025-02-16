use std::collections::HashMap;

pub struct PersistentIndexMap<K, V> {
    pub map: HashMap<K, (usize, V)>, // Maps keys to their persistent indices and values
    pub elements: Vec<Option<(K, V)>>, // Stores keys and values by index for iteration
    pub next_index: usize,           // Tracks the next available index
}

impl<K: Clone + Eq + std::hash::Hash, V: Clone> PersistentIndexMap<K, V> {
    /// Create a new `PersistentIndexMap`
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            elements: Vec::new(),
            next_index: 0,
        }
    }

    /// Insert a key-value pair, returning the persistent index.
    /// If the key already exists, it does not overwrite the value and returns the existing index.
    pub fn insert(&mut self, key: K, value: V) -> usize {
        if let Some(&(index, _)) = self.map.get(&key) {
            index // Return existing index if already present
        } else {
            let index = self.next_index;
            self.map.insert(key.clone(), (index, value.clone()));

            // Ensure `elements` vector is large enough to hold the new index
            if index >= self.elements.len() {
                self.elements.resize(index + 1, None);
            }
            self.elements[index] = Some((key, value));

            self.next_index += 1; // Increment for the next new element
            index
        }
    }

    /// Get the persistent index of a key, if it exists.
    pub fn get_index(&self, key: &K) -> Option<usize> {
        self.map.get(key).map(|&(index, _)| index)
    }

    /// Get the value associated with a key, if it exists.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key).map(|&(_, ref value)| value)
    }

    /// Get the mutable reference to the value, with a key, if it exists
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.map.get_mut(key).map(|&mut (_, ref mut value)| value)
    }

    /// Check if a key exists in the map.
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Remove a key-value pair by key.
    /// The persistent index remains in the internal structure but is marked as `None`.
    pub fn remove(&mut self, key: &K) -> bool {
        if let Some((index, _)) = self.map.remove(key) {
            self.elements[index] = None; // Mark the index as empty
            true
        } else {
            false
        }
    }

    /// Get the number of elements in the map.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate over all key-value pairs sorted by their persistent indices, with dense reindexed indices.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &K, &V)> {
        let mut entries: Vec<(usize, &K, &V)> = self
            .elements
            .iter()
            .enumerate() // Retain the original persistent index
            .filter_map(|(persistent_index, opt_entry)| {
                opt_entry
                    .as_ref()
                    .map(|(key, value)| (persistent_index, key, value))
            })
            .collect();

        // Sort by the persistent index (default for usize)
        entries.sort_by_key(|(persistent_index, _, _)| *persistent_index);

        // Re-index dense indices
        entries
            .into_iter()
            .enumerate()
            .map(|(dense_index, (_, key, value))| (dense_index, key, value))
    }
}
