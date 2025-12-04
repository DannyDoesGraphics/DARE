//!
//! # Asset System Module
//! This module provides the core functionality for managing assets within the engine.
//!
//! ## Geometry
//! `Geometry` represent the lowest level of unit of asset data, typically a descriptor defining location of data, format, offset, stride, etc.
//!
//! ## Mesh
//! A mesh is a collection of geometries that together form a complete 3D model. At a minimum, a mesh consists of a vertex and index geometry alongside a transform.
//!
//! ### Initialization
//! Meshes are by default, not loaded, but instead vacant on the GPU, until explicitly loaded via the asset system.
//! Meshes are loaded when the
//!
//! ## Asset Handles
//! - Mesh handles and geometry handles are used to uniquely identify and manage assets within the system
//! - Asset handles are 64-bit values where the lower 32 bits represent the asset ID and the upper 32 bits represent the generation of the asset.
//!
//!
//!
//! ## Streaming
//! - The asset system supports streaming of assets via chunked loaded (256kb chunks by default)
//! -

pub mod format;
pub mod geometry;
pub mod handle;
pub mod mesh;
pub mod stream;
pub use format::*;
pub use geometry::*;
pub use handle::*;
pub use mesh::*;

use std::collections::HashMap;
use std::sync::Arc;

/// Describes where the data is located
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataLocation {
    Url(String),
    File(std::path::PathBuf),
    Blob(Arc<[u8]>),
}

/// A simple LRU Cache for assets, kicks out least recently used assets when full
pub struct LRUCache {}

/// Asset manager is responsible for handling the high-level asset operations
#[derive(Debug)]
pub struct AssetManager {
    pub geometry_store: dare_containers::slot_map::SlotMap<Geometry, GeometryHandle>,
    pub mesh_store: dare_containers::slot_map::SlotMap<MeshAsset, MeshHandle>,
}

impl AssetManager {
    /// Creates a new asset manager
    pub fn new() -> Self {
        Self {
            geometry_store: dare_containers::slot_map::SlotMap::default(),
            mesh_store: dare_containers::slot_map::SlotMap::default(),
        }
    }

    /// Loads a glTF file and returns a handle to the unloaded meshes
    ///
    /// Meshes are not loaded onto the GPU until explicitly requested
    pub fn load_gltf(
        &mut self,
        commands: &mut bevy_ecs::prelude::Commands,
        path: &std::path::Path,
    ) {
        // Load a gltf files' meshes into asset manager
        let gltf = gltf::Gltf::open(path).expect("Failed to open gltf file");
        let blob: Option<Arc<[u8]>> = gltf.blob.as_ref().map(|b| Arc::from(b.as_slice()));
        let accessors: Vec<GeometryHandle> =
            gltf.accessors()
                .map(|accessor| {
                    if accessor.sparse().is_some() {
                        unimplemented!("Sparse accessors are not supported yet");
                    }

                    let buffer_view = accessor.view().expect("Accessor has no buffer view");
                    let buffer = buffer_view.buffer();
                    self.geometry_store.insert(Geometry {
                        location: match buffer.source() {
                            gltf::buffer::Source::Bin => DataLocation::Blob(blob.clone().expect(
                                "No blob data in gltf, but accessor references binary buffer",
                            )),
                            gltf::buffer::Source::Uri(uri) => {
                                if !uri.starts_with("data") {
                                    let mut path = path
                                        .parent()
                                        .expect("gltf has no parent directory")
                                        .to_path_buf();
                                    path.push(uri);
                                    DataLocation::File(path)
                                } else {
                                    unimplemented!("Data URIs are not supported yet")
                                }
                            }
                        },
                        format: match accessor.data_type() {
                            gltf::accessor::DataType::I8 => Format::U16,
                            gltf::accessor::DataType::U8 => Format::U16,
                            gltf::accessor::DataType::I16 => Format::U16,
                            gltf::accessor::DataType::U16 => Format::U16,
                            gltf::accessor::DataType::U32 => Format::U32,
                            gltf::accessor::DataType::F32 => match accessor.dimensions() {
                                gltf::accessor::Dimensions::Scalar => Format::F32,
                                gltf::accessor::Dimensions::Vec2 => Format::F32x2,
                                gltf::accessor::Dimensions::Vec3 => Format::F32x3,
                                gltf::accessor::Dimensions::Vec4 => Format::F32x4,
                                gltf::accessor::Dimensions::Mat2 => Format::F32x4,
                                _ => unimplemented!(),
                            },
                            _ => unimplemented!(),
                        },
                        offset: buffer_view.offset() as u64 + accessor.offset() as u64,
                        stride: buffer_view.stride().map(|s| s as u64),
                        max_size: accessor.count() as u64 * accessor.size() as u64,
                        count: accessor.count() as u64,
                    })
                })
                .collect();
        gltf.meshes()
            .map(|mesh| {
                mesh.primitives()
                    .map(|primitive| MeshAsset {
                        index_buffer: accessors[primitive
                            .indices()
                            .expect("All surfaces must have indices")
                            .index()],
                        vertex_buffer: accessors[primitive
                            .attributes()
                            .find(|(semantic, _)| semantic == gltf::Semantic::Positions)
                            .expect("All surfaces must have positions")
                            .1
                            .index()],
                        uv_buffers: HashMap::new(),
                    })
                    .collect::<Vec<MeshAsset>>()
            })
            .flatten()
            .collect::<Vec<MeshAsset>>();
    }
}
