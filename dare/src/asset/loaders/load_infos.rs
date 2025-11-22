use std::ops::Deref;

/// Contains basic primitives most if not all load_info structs may have

/// Define a chunk size in bytes to be streamed in at
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkSize(pub usize);

impl Default for ChunkSize {
    fn default() -> Self {
        Self { 0: 0 }
    }
}

impl Deref for ChunkSize {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
