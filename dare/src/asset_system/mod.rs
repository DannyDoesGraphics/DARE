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

#![allow(dead_code)]

pub mod format;
pub mod geometry;
pub mod handle;
pub mod mesh;
pub mod stream;
use dagal::sync::fence;
pub use format::*;
pub use geometry::*;
pub use handle::*;
pub use mesh::*;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use super::prelude::physics;
use bevy_ecs::prelude::*;

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
#[derive(Debug, Resource)]
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
        // transformations are nested, we need to bfs unwrap them
        let meshes_with_transformations: Vec<(gltf::Mesh, glam::Mat4)> = {
            let mut out: Vec<(gltf::Mesh, glam::Mat4)> = Vec::new();
            let mut queue: VecDeque<(gltf::Node, glam::Mat4)> = gltf
                .document
                .default_scene()
                .expect("No default scene set")
                .nodes()
                .map(|node| {
                    let t = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
                    (node, t)
                })
                .collect();
            while let Some((node, transform)) = queue.pop_front() {
                for child in node.children() {
                    let t = glam::Mat4::from_cols_array_2d(&child.transform().matrix());
                    queue.push_back((child, transform * t));
                }
                if let Some(mesh) = node.mesh() {
                    out.push((mesh, transform));
                }
            }
            out
        };

        let meshes = gltf.meshes()
            .flat_map(|mesh| {
                mesh.primitives()
                    .map(|primitive| {
                        let mut uv_buffers: HashMap<u32, GeometryHandle> = HashMap::new();
                        let mut vertex_buffer: Option<GeometryHandle> = None;
                        let mut normal_buffer: Option<GeometryHandle> = None;
                        for (semantic, accessor) in primitive.attributes() {
                            match semantic {
                                gltf::Semantic::Positions => {
                                    assert!(vertex_buffer.replace(accessors[accessor.index()]).is_none(), "Vertex buffer already exists");
                                }
                                gltf::Semantic::Normals => {
                                    assert!(normal_buffer.replace(accessors[accessor.index()]).is_none(), "Normal buffer already exists");
                                }
                                gltf::Semantic::TexCoords(index) => {
                                    assert!(uv_buffers.insert(index, accessors[accessor.index()]).is_none(), "UV buffer already exists");
                                }
                                _ => {}
                            }
                        }
                        self.mesh_store.insert(MeshAsset {
                            index_buffer: accessors[primitive.indices().unwrap().index()],
                            vertex_buffer: vertex_buffer.unwrap(),
                            normal_buffer: normal_buffer.unwrap(),
                            uv_buffers,
                        })
                    })
                    .collect::<Vec<MeshHandle>>()
            })
            .collect::<Vec<MeshHandle>>();
        tracing::info!("Geometries loaded: {}", accessors.len());
        tracing::info!("Meshes loaded: {}", meshes.len());
        
        for (mesh, transform) in meshes_with_transformations {
            let transform = physics::components::Transform::from(transform);
            let mesh = meshes[mesh.index()];
            commands.spawn((mesh, transform));
        }
    }
}
