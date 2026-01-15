use std::{ops::Deref, sync::{Arc, atomic::AtomicU8}};

/// Describes where the underlying bytes are located.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataLocation {
    Url(String),
    File(std::path::PathBuf),
    Blob(Arc<[u8]>),
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
#[repr(u8)]
pub enum ResidentState {
    Empty = 0u8,
    Loading = 1u8,
    ResidentGPU = 2u8,
    Unloading = 3u8,
    Unloaded = 4u8
}

/// A structure representing metadata to load a geometry
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct GeometryDescription {
    pub location: DataLocation,
    pub format: crate::Format,
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
            ttl: std::sync::atomic::AtomicU16::from(0),
        }
    }
}
