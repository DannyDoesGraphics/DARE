use crate::prelude as dare;
use crate::prelude::engine;
use crate::prelude::render::InnerRenderServerRequest::Delta;
use crate::prelude::render::RenderServerAssetRelationDelta;
use crate::render2::prelude::InnerRenderServerRequest;
use crate::render2::server::IrSend;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dare::asset2 as asset;
use gltf;
use gltf::accessor::DataType;
use gltf::buffer::Source;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::sync::Arc;

/// This is similar to [`gltf::Semantic`], but includes the Index
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum GltfSemantics {
    Index,
    Accessor(gltf::Semantic),
    UVs,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Required<T> {
    No(T),
    Yes(T),
}

/// Expected semantics we want to have
pub const EXPECTED_SEMANTICS: [Required<GltfSemantics>; 4] = [
    Required::Yes(GltfSemantics::Index),
    Required::Yes(GltfSemantics::Accessor(gltf::Semantic::Positions)),
    Required::No(GltfSemantics::Accessor(gltf::Semantic::Normals)),
    Required::No(GltfSemantics::UVs),
];

/// Handles gltf loading
pub struct GLTFLoader {
    /// Location of the .gltf file
    path: std::path::PathBuf,
}

impl GLTFLoader {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }

    pub fn load(
        commands: &mut becs::Commands,
        asset_server: &dare::asset2::server::AssetServer,
        send: IrSend,
        path: std::path::PathBuf,
    ) -> Result<()> {
        let gltf: gltf::Gltf = gltf::Gltf::open(path.clone())?;
        let blob: Option<Arc<[u8]>> = gltf
            .blob
            .clone()
            .map(|blob| Arc::from(blob.into_boxed_slice()));
        let buffer_metadatas = gltf
            .buffers()
            .map(|buffer| {
                let location: asset::MetaDataLocation = match buffer.source() {
                    Source::Bin => {
                        if let Some(blob) = blob.clone() {
                            asset::MetaDataLocation::Memory(blob)
                        } else {
                            return Err::<_, anyhow::Error>(anyhow::anyhow!(
                                "Expected blob, got None"
                            ));
                        }
                    }
                    Source::Uri(uri) => {
                        if !uri.starts_with("data") {
                            let mut path = path.parent().unwrap().to_path_buf();
                            path.push(std::path::PathBuf::from(uri));
                            asset::MetaDataLocation::FilePath(path)
                        } else {
                            unimplemented!()
                        }
                    }
                };
                Ok(asset::assets::BufferMetaData {
                    location,
                    offset: 0,
                    length: buffer.length(),
                    stride: None,
                    format: dare::render::util::Format::new(
                        dare::render::util::ElementFormat::U8,
                        1,
                    ),
                    element_count: 0,
                })
            })
            .collect::<Vec<Result<asset::assets::BufferMetaData>>>();
        let accessors_metadata = gltf
            .accessors()
            .map(|accessor| {
                if accessor.sparse().is_some() {
                    return panic!("Does not support sparse data");
                } else if let Some(view) = accessor.view() {
                    if let Ok(buffer_metadata) =
                        buffer_metadatas.get(view.buffer().index()).unwrap()
                    {
                        let mut buffer_metadata = buffer_metadata.clone();
                        buffer_metadata.length = view.length();
                        buffer_metadata.stride = view.stride();
                        buffer_metadata.offset = view.offset() + accessor.offset();
                        buffer_metadata.element_count = accessor.count();
                        buffer_metadata.format = dare::render::util::Format::new(
                            dare::render::util::ElementFormat::from(accessor.data_type()),
                            accessor.dimensions().multiplicity(),
                        );
                        {
                            let mut buffer_metadata = buffer_metadata.clone();
                            buffer_metadata.format = dare::render::util::Format::new(
                                dare::render::util::ElementFormat::U8,
                                1,
                            );
                            let vec: Vec<u8> = (u8::MIN..=u8::MAX).collect();
                            let len = vec.len();
                            let length = size_of_val(&vec);
                            buffer_metadata.location =
                                asset::MetaDataLocation::Memory(Arc::from(vec));
                            buffer_metadata.length = length;
                            buffer_metadata.stride = Some(4);
                            buffer_metadata.offset = 16;
                            buffer_metadata.element_count = len;
                            buffer_metadata.format = dare::render::util::Format::new(
                                dare::render::util::ElementFormat::U8,
                                3,
                            );
                            asset_server
                                .entry::<dare::asset2::assets::Buffer>(buffer_metadata.clone());
                        }
                        asset_server.entry::<dare::asset2::assets::Buffer>(buffer_metadata.clone())
                    } else {
                        panic!("No metadata found at {}", view.buffer().index())
                    }
                } else {
                    unimplemented!()
                }
            })
            .collect::<Vec<asset::AssetHandle<asset::assets::Buffer>>>();
        // make sure we pass the proper transform information
        let mut meshes: Vec<(gltf::Mesh, glam::Mat4)> = Vec::new();
        {
            // Root nodes
            let mut stack: VecDeque<(gltf::Node, glam::Mat4)> = gltf
                .document
                .default_scene()
                .unwrap()
                .nodes()
                .map(|node| (node, glam::Mat4::IDENTITY))
                .collect();
            while let Some((node, transform)) = stack.pop_front() {
                // create mesh
                if let Some(mesh) = node.mesh() {
                    meshes.push((mesh.clone(), transform));
                }
                {
                    // update transform and update stack
                    let transform =
                        transform * glam::Mat4::from_cols_array_2d(&node.transform().matrix());
                    let mut children: VecDeque<(gltf::Node, glam::Mat4)> =
                        node.children().map(|node| (node, transform)).collect();
                    stack.append(&mut children);
                }
            }
        }
        let meshes: Vec<engine::Mesh> = meshes
            .into_iter()
            .flat_map(|(mesh, transform)| {
                let mut surfaces = Vec::new();
                for primitive in mesh.primitives() {
                    // retrieve all required prims
                    //commands.spawn();
                    let mut surface_builder = engine::components::SurfaceBuilder::default();
                    let uv_indices: Vec<u32> = primitive
                        .attributes()
                        .flat_map(|(attr, _)| match attr {
                            gltf::Semantic::TexCoords(i) => Some(i),
                            _ => None,
                        })
                        .collect();
                    // Maps from uv index to uv position
                    let mut uv_mappings: Vec<(u32, u32)> = {
                        let mut index = 0u32;
                        primitive
                            .attributes()
                            .flat_map(|(attr, _)| match attr {
                                gltf::Semantic::TexCoords(i) => {
                                    let ret = Some((i, index));
                                    index += 1;
                                    ret
                                }
                                _ => None,
                            })
                            .collect()
                    };
                    uv_mappings.sort_by(|(_, a), (_, b)| a.cmp(b));
                    for semantic in EXPECTED_SEMANTICS.iter() {
                        let is_required = match semantic {
                            Required::No(_) => false,
                            Required::Yes(_) => true,
                        };
                        let semantic = match semantic {
                            Required::No(semantic) => semantic,
                            Required::Yes(semantic) => semantic,
                        };
                        match semantic {
                            GltfSemantics::Index => match primitive.indices() {
                                None => {
                                    if is_required {
                                        return Err(anyhow::anyhow!(
                                            "Missing indices in primitive, got None"
                                        ));
                                    }
                                }
                                Some(accessor) => {
                                    // # of indices
                                    surface_builder.index_count = accessor.count();
                                    surface_builder.first_index = 0;
                                    surface_builder.index_buffer =
                                        accessors_metadata.get(accessor.index()).cloned()
                                }
                            },
                            GltfSemantics::Accessor(semantic) => match primitive.get(semantic) {
                                None => {
                                    if is_required {
                                        return Err(anyhow::anyhow!(
                                            "Missing accessor {:?}, got NULL",
                                            semantic
                                        ));
                                    }
                                }
                                Some(accessor) => {
                                    use gltf::Semantic::*;
                                    match semantic {
                                        Positions => {
                                            surface_builder.vertex_count = accessor.count();
                                            surface_builder.vertex_buffer =
                                                accessors_metadata.get(accessor.index()).cloned()
                                        }
                                        Normals => {
                                            surface_builder.normal_buffer =
                                                accessors_metadata.get(accessor.index()).cloned()
                                        }
                                        Tangents => {
                                            surface_builder.tangent_buffer =
                                                accessors_metadata.get(accessor.index()).cloned()
                                        }
                                        Colors(_) => {}
                                        TexCoords(_) => {}
                                        Joints(_) => {}
                                        Weights(_) => {}
                                        _ => {}
                                    };
                                }
                            },
                            GltfSemantics::UVs => {
                                for (uv_index, index) in uv_mappings.iter() {
                                    primitive
                                        .get(&gltf::Semantic::TexCoords(*uv_index))
                                        .and_then(|accessor| {
                                            accessors_metadata.get(accessor.index()).cloned()
                                        });
                                }
                            }
                        };
                    }
                    let surface = surface_builder.build();
                    // decompose
                    let (scale, rotation, translation) = transform.to_scale_rotation_translation();
                    surfaces.push(engine::Mesh {
                        surface,
                        transform: dare::physics::components::Transform {
                            scale,
                            rotation,
                            translation,
                        },
                    });
                }
                Ok(surfaces)
            })
            .flatten()
            .collect::<Vec<engine::Mesh>>();

        commands.spawn_batch(meshes.clone().into_iter());
        Ok(())
    }
}
