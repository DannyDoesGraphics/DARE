/// Describes a set of bytes and how to access and read
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetBuffer {
    /// Size in bytes expected from the buffer from offset ie [offset, offset + size]
    pub size: u64,
    /// Offset into buffer
    pub offset: u64,
    /// Stride between elements
    pub stride: u64,
    /// Number of elements
    pub element_count: u64,
    /// Format of elements
    pub format: super::super::Format,
}
