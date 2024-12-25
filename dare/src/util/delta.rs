use std::fmt::{Debug, Formatter, Pointer};

/// Describes defaults for deltas
pub enum DeltaHash<T> {
    Added(T),
    /// Uses a hashing algorithm to specify which has been removed
    RemovedHash(u64),
}

impl<T: Clone> Clone for DeltaHash<T> {
    fn clone(&self) -> Self {
        self.clone()
    }
}

impl<T: Debug> Debug for DeltaHash<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.fmt(f)
    }
}
