use std::sync::Arc;

/// Describes all formats supported by geometry assets.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Format {
    U16,
    U32,
    U64,
    F32,
    F64,
    F32x2,
    F32x3,
    F32x4,
    F64x2,
    F64x3,
    F64x4,
    UNKNOWN,
}

impl Format {
    pub fn size_in_bytes(&self) -> usize {
        match self {
            Format::U16 => 2,
            Format::U32 => 4,
            Format::U64 => 8,
            Format::F32 => 4,
            Format::F64 => 8,
            Format::F32x2 => 8,
            Format::F32x3 => 12,
            Format::F32x4 => 16,
            Format::F64x2 => 16,
            Format::F64x3 => 24,
            Format::F64x4 => 32,
            Format::UNKNOWN => 0,
        }
    }
}

/// Describes where the underlying bytes are located.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataLocation {
    Url(String),
    File(std::path::PathBuf),
    Blob(Arc<[u8]>),
}

/// A structure representing geometric data in the asset system.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Geometry {
    pub location: DataLocation,
    pub format: Format,
    pub offset: u64,
    /// If None, data is tightly packed
    pub stride: Option<u64>,
    /// \# of elements defined as [`Geometry::format`]
    pub count: u64,
}
