use std::{ops::Deref, sync::Arc};

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

/// Always represents an instance of [`Geometry`], and is backed by every [`crate::GeometryHandle`] in [`crate::Assets`]
///
/// Defines the resident state of geometries
#[derive(Debug)]
pub struct GeometryRuntime {
    /// See [`ResidentState`]
    pub residency: std::sync::atomic::AtomicU8,
    /// Time to live remaining on geometry
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
