#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkDesc {
    pub offset: u64,
    pub size: u64,
    pub dst_offset: u64,
}
