#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkDesc {
    pub offset: u64,     // Offset in the source asset file
    pub size: u64,       // Size of the chunk to stream
    pub dst_offset: u64, // Where to place the chunk in the destination buffer
}

pub enum StreamState<Handle> {
    Vacant,
    Loading,
    Resident(Handle),
    Failed,
}

/// A singular unit request of streaming work
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct UnitStream {}
