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
            Format::F32x2 => 4 * 2,
            Format::F32x3 => 4 * 3,
            Format::F32x4 => 4 * 4,
            Format::F64x2 => 8 * 2,
            Format::F64x3 => 8 * 3,
            Format::F64x4 => 8 * 4,
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

/// A structure representing metadata to load a geometry
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

/// Always represents an instance of [`Geometry`], and is backed by every [`crate::GeometryHandle`] in [`crate:AssetManager`]
/// 
/// Defines the resident state of geometries
#[derive(Debug)]
pub struct GeometryRuntime {
    pub residency: std::sync::atomic::AtomicU8,
    pub ttl: std::sync::atomic::AtomicU16,
}

impl Default for GeometryRuntime {
    /// By default, constructs a runtime that will be destroyed instantly, it is expected you set the ttl remaining
    fn default() -> Self {
        Self {
            residency: std::sync::atomic::AtomicU8::from(0),
            ttl: std::sync::atomic::AtomicU16::from(0)
        }
    }
}