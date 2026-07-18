use crate::{DataLocation, Format};
use bevy_ecs::prelude::*;
use std::collections::{HashMap, VecDeque};

impl crate::Assets<crate::Mesh> {
    /// Loads a glTF file and spawns entities containing `(AssetHandle<Mesh>, dare_physics::Transform)`.
    pub fn load_gltf(
        &mut self,
        commands: &mut Commands,
        buffers: &mut crate::Assets<crate::Buffer>,
        path: &std::path::Path,
    ) {
        let gltf = gltf::Gltf::open(path).expect("Failed to open gltf file");
        let bin_chunk_offset: Option<usize> = gltf.blob.is_some().then(|| {
            use std::io::Read;
            let mut file = std::fs::File::open(path).expect("Failed to open glb file");
            let mut header = [0u8; 16];
            file.read_exact(&mut header)
                .expect("Failed to read glb binary chunk header");
            let json_chunk_length = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;
            20 + json_chunk_length + 8
        });

        let accessors: Vec<crate::AssetHandle<crate::Buffer>> =
            gltf.accessors()
                .map(|accessor| {
                    if accessor.sparse().is_some() {
                        unimplemented!("Sparse accessors are not supported yet");
                    }

                    let buffer_view = accessor.view().expect("Accessor has no buffer view");
                    let buffer = buffer_view.buffer();

                    let format = match accessor.data_type() {
                        gltf::accessor::DataType::I8 => unimplemented!(),
                        gltf::accessor::DataType::U8 => match accessor.dimensions() {
                            gltf::accessor::Dimensions::Scalar => Format::U8,
                            gltf::accessor::Dimensions::Vec3 => Format::U8x3,
                            gltf::accessor::Dimensions::Vec4 => Format::U8x4,
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
                    };

                    let stride = buffer_view.stride().map(|s| s as usize);
                    let span = match stride {
                        Some(stride) => {
                            accessor.count().saturating_sub(1) * stride + format.size_in_bytes()
                        }
                        None => accessor.count() * format.size_in_bytes(),
                    };
                    let offset = buffer_view.offset() + accessor.offset();

                    let buffer = crate::Buffer {
                        location: match buffer.source() {
                            gltf::buffer::Source::Bin => {
                                let bin_chunk_offset = bin_chunk_offset
                                    .expect("Buffer references BIN chunk, but glTF has none");
                                DataLocation::File {
                                    path: path.to_path_buf(),
                                    offset: bin_chunk_offset + offset,
                                    length: span,
                                }
                            }
                            gltf::buffer::Source::Uri(uri) => {
                                if !uri.starts_with("data") {
                                    let mut resolved = path
                                        .parent()
                                        .expect("gltf has no parent directory")
                                        .to_path_buf();
                                    resolved.push(uri);
                                    DataLocation::File {
                                        path: resolved,
                                        offset,
                                        length: span,
                                    }
                                } else {
                                    unimplemented!("Data URIs are not supported yet")
                                }
                            }
                        },
                        format,
                        stride: stride.map(|s| s as u64),
                        count: accessor.count() as u64,
                    };
                    let name = accessor
                        .name()
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("accessor{}", accessor.index()));
                    buffers.insert_named(buffer, Some(name))
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

            // BFS to unravel the transformation tree
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
                let mesh_name = mesh
                    .name()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("mesh{}", mesh.index()));
                mesh.primitives()
                    .map(|primitive| {
                        let mut uv_buffers: HashMap<u32, crate::AssetHandle<crate::Buffer>> =
                            HashMap::new();
                        let mut vertex_buffer: Option<crate::AssetHandle<crate::Buffer>> = None;
                        let mut normal_buffer: Option<crate::AssetHandle<crate::Buffer>> = None;
                        let mut bounding_box: Option<dare_physics::BoundingBox> = None;

                        for (semantic, accessor) in primitive.attributes() {
                            match semantic {
                                gltf::Semantic::Positions => {
                                    assert!(
                                        vertex_buffer
                                            .replace(accessors[accessor.index()].clone())
                                            .is_none(),
                                        "Vertex buffer already exists"
                                    );

                                    let min_arr = accessor.min().unwrap();
                                    let max_arr = accessor.max().unwrap();
                                    let min_arr = min_arr.as_array().unwrap();
                                    let max_arr = max_arr.as_array().unwrap();
                                    bounding_box = Some(dare_physics::BoundingBox::new(
                                        glam::Vec3::new(
                                            min_arr[0].as_f64().unwrap() as f32,
                                            min_arr[1].as_f64().unwrap() as f32,
                                            min_arr[2].as_f64().unwrap_or_default() as f32,
                                        ),
                                        glam::Vec3::new(
                                            max_arr[0].as_f64().unwrap() as f32,
                                            max_arr[1].as_f64().unwrap() as f32,
                                            max_arr[2].as_f64().unwrap_or_default() as f32,
                                        ),
                                    ));
                                }
                                gltf::Semantic::Normals => {
                                    assert!(
                                        normal_buffer
                                            .replace(accessors[accessor.index()].clone())
                                            .is_none(),
                                        "Normal buffer already exists"
                                    );
                                }
                                gltf::Semantic::TexCoords(index) => {
                                    assert!(
                                        uv_buffers
                                            .insert(index, accessors[accessor.index()].clone())
                                            .is_none(),
                                        "UV buffer already exists"
                                    );
                                }
                                _ => {}
                            }
                        }

                        (
                            self.insert_named(
                                crate::Mesh {
                                    index_buffer: accessors[primitive.indices().unwrap().index()]
                                        .clone(),
                                    vertex_buffer: vertex_buffer.unwrap(),
                                    normal_buffer: normal_buffer.unwrap(),
                                    uv_buffers,
                                },
                                Some(format!("{mesh_name}[{}]", primitive.index())),
                            ),
                            bounding_box.unwrap(),
                        )
                    })
                    .collect::<Vec<(crate::AssetHandle<crate::Mesh>, dare_physics::BoundingBox)>>()
            })
            .collect::<Vec<(crate::AssetHandle<crate::Mesh>, dare_physics::BoundingBox)>>();

        tracing::info!("Loaded {} geometries", accessors.len());
        tracing::info!("Loaded {} meshes", meshes.len());

        // per-mesh start index into the primitive-flattened `meshes`
        let mesh_primitive_offsets: Vec<usize> = gltf
            .meshes()
            .scan(0usize, |offset, mesh| {
                let start = *offset;
                *offset += mesh.primitives().count();
                Some(start)
            })
            .collect();

        for (mesh, transform) in meshes_with_transformations {
            let start = mesh_primitive_offsets[mesh.index()];
            for i in 0..mesh.primitives().count() {
                let (mesh, bounding_box) = meshes[start + i].clone();
                commands.spawn((mesh, bounding_box, dare_physics::Transform::from(transform)));
            }
        }
    }
}

#[cfg(test)]
mod glb_offset_tests {
    #[test]
    fn bin_chunk_offset_matches_real_glb() {
        let json = br#"{"asset":{"version":"2.0"}}"#.to_vec();
        let bin = b"hello world, this is bin data".to_vec();

        let glb = gltf::binary::Glb {
            header: gltf::binary::Header {
                magic: *b"glTF",
                version: 2,
                length: 0,
            },
            json: json.into(),
            bin: Some(bin.clone().into()),
        };
        let bytes = glb.to_vec().unwrap();

        let tmp = std::env::temp_dir().join("dare_glb_offset_test.glb");
        std::fs::write(&tmp, &bytes).unwrap();

        let json_chunk_length =
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let computed_offset = 20 + json_chunk_length + 8;

        assert_eq!(&bytes[computed_offset..computed_offset + bin.len()], &bin[..]);

        let parsed = gltf::Gltf::open(&tmp).unwrap();
        assert_eq!(&parsed.blob.unwrap()[..bin.len()], &bin[..]);

        std::fs::remove_file(&tmp).ok();
    }
}
