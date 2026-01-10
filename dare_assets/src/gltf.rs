use crate::{
    AssetManager, DataLocation, Format, GeometryDescription, GeometryDescriptionHandle, MeshAsset,
    MeshHandle,
};
use bevy_ecs::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

impl AssetManager {
    /// Loads a glTF file and spawns entities containing `(MeshHandle, dare_physics::Transform)`.
    pub fn load_gltf(&mut self, commands: &mut Commands, path: &std::path::Path) {
        let gltf = gltf::Gltf::open(path).expect("Failed to open gltf file");
        let blob: Option<Arc<[u8]>> = gltf.blob.as_ref().map(|b| Arc::from(b.as_slice()));

        let accessors: Vec<GeometryDescriptionHandle> =
            gltf.accessors()
                .map(|accessor| {
                    if accessor.sparse().is_some() {
                        unimplemented!("Sparse accessors are not supported yet");
                    }

                    let buffer_view = accessor.view().expect("Accessor has no buffer view");
                    let buffer = buffer_view.buffer();

                    self.create_geometry(GeometryDescription {
                        location: match buffer.source() {
                            gltf::buffer::Source::Bin => DataLocation::Blob(blob.clone().expect(
                                "No blob data in gltf, but accessor references binary buffer",
                            )),
                            gltf::buffer::Source::Uri(uri) => {
                                if !uri.starts_with("data") {
                                    let mut resolved = path
                                        .parent()
                                        .expect("gltf has no parent directory")
                                        .to_path_buf();
                                    resolved.push(uri);
                                    DataLocation::File(resolved)
                                } else {
                                    unimplemented!("Data URIs are not supported yet")
                                }
                            }
                        },
                        format: match accessor.data_type() {
                            gltf::accessor::DataType::I8 => unimplemented!(),
                            gltf::accessor::DataType::U8 => match accessor.dimensions() {
                                gltf::accessor::Dimensions::Scalar => Format::U8,
                                _ => unimplemented!(),
                            },
                            gltf::accessor::DataType::I16 => unimplemented!(),
                            gltf::accessor::DataType::U16 => match accessor.dimensions() {
                                gltf::accessor::Dimensions::Scalar => Format::U16,
                                _ => unimplemented!(),
                            },
                            gltf::accessor::DataType::U32 => match accessor.dimensions() {
                                gltf::accessor::Dimensions::Scalar => Format::U32,
                                _ => unimplemented!(),
                            },
                            gltf::accessor::DataType::F32 => match accessor.dimensions() {
                                gltf::accessor::Dimensions::Scalar => Format::F32,
                                gltf::accessor::Dimensions::Vec2 => Format::F32x2,
                                gltf::accessor::Dimensions::Vec3 => Format::F32x3,
                                gltf::accessor::Dimensions::Vec4 => Format::F32x4,
                                gltf::accessor::Dimensions::Mat2 => unimplemented!(),
                                _ => unimplemented!(),
                            },
                        },
                        offset: buffer_view.offset() as u64 + accessor.offset() as u64,
                        stride: buffer_view.stride().map(|s| s as u64),
                        count: accessor.count() as u64,
                    })
                })
                .collect();

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

        let meshes = gltf
            .meshes()
            .flat_map(|mesh| {
                mesh.primitives()
                    .map(|primitive| {
                        let mut uv_buffers: HashMap<u32, GeometryDescriptionHandle> =
                            HashMap::new();
                        let mut vertex_buffer: Option<GeometryDescriptionHandle> = None;
                        let mut normal_buffer: Option<GeometryDescriptionHandle> = None;

                        for (semantic, accessor) in primitive.attributes() {
                            match semantic {
                                gltf::Semantic::Positions => {
                                    assert!(
                                        vertex_buffer
                                            .replace(accessors[accessor.index()])
                                            .is_none(),
                                        "Vertex buffer already exists"
                                    );
                                }
                                gltf::Semantic::Normals => {
                                    assert!(
                                        normal_buffer
                                            .replace(accessors[accessor.index()])
                                            .is_none(),
                                        "Normal buffer already exists"
                                    );
                                }
                                gltf::Semantic::TexCoords(index) => {
                                    assert!(
                                        uv_buffers
                                            .insert(index, accessors[accessor.index()])
                                            .is_none(),
                                        "UV buffer already exists"
                                    );
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
            commands.spawn((
                meshes[mesh.index()],
                dare_physics::Transform::from(transform),
            ));
        }
    }
}
