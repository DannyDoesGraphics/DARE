use crate::prelude as dare;
use crate::prelude::engine;
use anyhow::Result;
use bevy_ecs::prelude as becs;
use dare::asset;
use gltf;
use gltf::texture::{MagFilter, MinFilter};
use std::collections::VecDeque;
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
        asset_server: &dare::asset::server::AssetServer,
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
                    gltf::buffer::Source::Bin => {
                        if let Some(blob) = blob.clone() {
                            asset::MetaDataLocation::Memory(blob)
                        } else {
                            return Err::<_, anyhow::Error>(anyhow::anyhow!(
                                "Expected blob, got None"
                            ));
                        }
                    }
                    gltf::buffer::Source::Uri(uri) => {
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
                    stored_format: dare::render::util::Format::new(
                        dare::render::util::ElementFormat::U8,
                        1,
                    ),
                    element_count: 0,
                    name: String::new(),
                })
            })
            .collect::<Vec<Result<asset::assets::BufferMetaData>>>();
        let accessors_metadata: Vec<dare::asset::assets::BufferMetaData> = gltf
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
                        buffer_metadata.stored_format = dare::render::util::Format::new(
                            dare::render::util::ElementFormat::from(accessor.data_type()),
                            accessor.dimensions().multiplicity(),
                        );
                        //asset_server.entry::<dare::asset::assets::Buffer>(buffer_metadata.clone())
                        buffer_metadata
                    } else {
                        panic!("No metadata found at {}", view.buffer().index())
                    }
                } else {
                    unimplemented!()
                }
            })
            .collect::<Vec<_>>();
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
                {
                    // update transform and update stack
                    let transform =
                        transform * glam::Mat4::from_cols_array_2d(&node.transform().matrix());
                    let mut children: VecDeque<(gltf::Node, glam::Mat4)> =
                        node.children().map(|node| (node, transform)).collect();
                    if let Some(mesh) = node.mesh() {
                        meshes.push((mesh.clone(), transform));
                    }
                    stack.append(&mut children);
                }
            }
        }
        let sampler: Vec<engine::components::Sampler> = gltf
            .document
            .samplers()
            .map(|sampler| {
                use gltf::json::texture::WrappingMode;
                engine::components::Sampler {
                    wrapping_mode: (
                        match sampler.wrap_s() {
                            WrappingMode::ClampToEdge => {
                                crate::render::util::WrappingMode::ClampToEdge
                            }
                            WrappingMode::MirroredRepeat => {
                                crate::render::util::WrappingMode::MirroredRepeat
                            }
                            WrappingMode::Repeat => crate::render::util::WrappingMode::Repeat,
                        },
                        match sampler.wrap_t() {
                            WrappingMode::ClampToEdge => {
                                crate::render::util::WrappingMode::ClampToEdge
                            }
                            WrappingMode::MirroredRepeat => {
                                crate::render::util::WrappingMode::MirroredRepeat
                            }
                            WrappingMode::Repeat => crate::render::util::WrappingMode::Repeat,
                        },
                    ),
                    min_filter: match sampler.min_filter() {
                        None => dare::render::util::ImageFilter::Linear,
                        Some(v) => match v {
                            MinFilter::Nearest
                            | MinFilter::NearestMipmapNearest
                            | MinFilter::NearestMipmapLinear
                            | MinFilter::LinearMipmapNearest => {
                                dare::render::util::ImageFilter::Nearest
                            }
                            MinFilter::Linear | MinFilter::LinearMipmapLinear => {
                                dare::render::util::ImageFilter::Linear
                            }
                        },
                    },
                    mag_filter: match sampler.mag_filter() {
                        None => dare::render::util::ImageFilter::Linear,
                        Some(v) => match v {
                            MagFilter::Nearest => dare::render::util::ImageFilter::Nearest,
                            MagFilter::Linear => dare::render::util::ImageFilter::Linear,
                        },
                    },
                }
            })
            .collect::<Vec<_>>();

        let textures: Vec<engine::components::Texture> = gltf
            .document
            .textures()
            .enumerate()
            .map(|(index, texture)| {
                let location = match texture.source().source() {
                    gltf::image::Source::Uri { uri, .. } => {
                        let parent = path.parent().unwrap();
                        dare::asset::MetaDataLocation::FilePath(parent.join(uri))
                    }
                    _ => unimplemented!(),
                };
                let sampler = engine::components::Sampler {
                    wrapping_mode: (
                        dare::render::util::WrappingMode::from(texture.sampler().wrap_s()),
                        dare::render::util::WrappingMode::from(texture.sampler().wrap_s()),
                    ),
                    min_filter: dare::render::util::ImageFilter::from(
                        texture.sampler().min_filter().unwrap_or(MinFilter::Nearest),
                    ),
                    mag_filter: dare::render::util::ImageFilter::from(
                        texture.sampler().mag_filter().unwrap_or(MagFilter::Nearest),
                    ),
                };
                let texture = dare::asset::assets::ImageMetaData {
                    location,
                    name: texture
                        .name()
                        .map(|n| n.to_string())
                        .unwrap_or(format!("Texture {}", texture.index()).to_string()),
                };
                let asset_handle: dare::asset::AssetHandle<dare::asset::assets::Image> =
                    asset_server.entry(texture);
                if asset_server.get_state(&asset_handle.clone().into_untyped_handle())
                    == Some(crate::asset::asset_state::AssetState::Unloaded)
                {
                    if let Err(e) =
                        asset_server.transition_loading(&asset_handle.clone().into_untyped_handle())
                    {
                        panic!("Failed to load: {e}");
                    }
                }
                engine::components::Texture {
                    asset_handle,
                    sampler,
                }
            })
            .collect::<Vec<engine::components::Texture>>();
        let mut mesh_count: usize = 0;
        let meshes: Vec<engine::components::Mesh> = meshes
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
                    let mut bounding_box: Option<dare::render::components::bounding_box::BoundingBox> =
                        None;
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
                                    let handle: Option<
                                        dare::asset::AssetHandle<dare::asset::assets::Buffer>,
                                    > = accessors_metadata.get(accessor.index()).cloned().map(
                                        |mut m| {
                                            m.format = dare::render::util::Format::new(
                                                dare::render::util::ElementFormat::U32,
                                                1,
                                            );
                                            m.name.push_str(&format!("Index buffer {} for surface {}", accessor.index(), mesh.name().unwrap_or(&mesh.index().to_string()) ));
                                            let handle = asset_server.entry(m.clone());
                                            if asset_server.get_state(&handle.clone().into_untyped_handle()) == Some(crate::asset::asset_state::AssetState::Unloaded) {
                                                if let Err(e) = asset_server.transition_loading(&handle.clone().into_untyped_handle()) {
                                                    tracing::warn!("Failed to load: {e}");
                                                }
                                            }
                                            handle
                                        },
                                    );
                                    surface_builder.index_buffer = handle;
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
                                            let handle: Option<
                                                dare::asset::AssetHandle<
                                                    dare::asset::assets::Buffer,
                                                >,
                                            > = accessors_metadata
                                                .get(accessor.index())
                                                .cloned()
                                                .map(|mut m| {
                                                    m.format = dare::render::util::Format::new(
                                                        dare::render::util::ElementFormat::F32,
                                                        3,
                                                    );
                                                    m.name.push_str(&format!("Vertex buffer {} for surface {}", accessor.index(), mesh.name().unwrap_or(&mesh.index().to_string()) ));
                                                    accessor.name().map(|name| m.name.push_str(name));
                                                    let handle = asset_server.entry(m.clone());
                                                    if asset_server.get_state(&handle.clone().into_untyped_handle()) == Some(crate::asset::asset_state::AssetState::Unloaded) {
                                                        if let Err(e) = asset_server.transition_loading(&handle.clone().into_untyped_handle()) {
                                                            tracing::warn!("Failed to load: {e}");
                                                        }
                                                    }
                                                    handle
                                                });
                                            surface_builder.vertex_count = accessor.count();
                                            surface_builder.vertex_buffer = handle;
                                            if let (Some(min), Some(max)) = (
                                                accessor.min().map(|v| v.as_array().cloned()).flatten(),
                                                accessor.max().map(|v| v.as_array().cloned()).flatten(),
                                            ) {
                                                let min = glam::Vec3::new(
                                                    min[0].as_f64().unwrap() as f32,
                                                    min[1].as_f64().unwrap() as f32,
                                                    min[2].as_f64().unwrap() as f32,
                                                );
                                                let max = glam::Vec3::new(
                                                    max[0].as_f64().unwrap() as f32,
                                                    max[1].as_f64().unwrap() as f32,
                                                    max[2].as_f64().unwrap() as f32,
                                                );
                                                bounding_box = Some(dare::render::components::bounding_box::BoundingBox {
                                                    min,
                                                    max,
                                                })
                                            }
                                        }
                                        Normals => {
                                            let handle: Option<
                                                dare::asset::AssetHandle<
                                                    dare::asset::assets::Buffer,
                                                >,
                                            > = accessors_metadata
                                                .get(accessor.index())
                                                .cloned()
                                                .map(|mut m| {
                                                    m.format = dare::render::util::Format::new(
                                                        dare::render::util::ElementFormat::F32,
                                                        3,
                                                    );
                                                    m.name.push_str(&format!("Normal buffer {} for surface {}", accessor.index(), mesh.name().unwrap_or(&mesh.index().to_string()) ));

                                                    accessor.name().map(|name| m.name.push_str(name));
                                                    let handle = asset_server.entry(m.clone());
                                                    if asset_server.get_state(&handle.clone().into_untyped_handle()) == Some(crate::asset::asset_state::AssetState::Unloaded) {
                                                        if let Err(e) = asset_server.transition_loading(&handle.clone().into_untyped_handle()) {
                                                            tracing::warn!("Failed to load: {e}");
                                                        }
                                                    }
                                                    handle
                                                });
                                            surface_builder.normal_buffer = handle;
                                        }
                                        Tangents => {
                                            let handle: Option<
                                                dare::asset::AssetHandle<
                                                    dare::asset::assets::Buffer,
                                                >,
                                            > = accessors_metadata
                                                .get(accessor.index())
                                                .cloned()
                                                .map(|mut m| {
                                                    m.format = dare::render::util::Format::new(
                                                        dare::render::util::ElementFormat::F32,
                                                        3,
                                                    );
                                                    m.name.push_str(&format!("Tangent buffer {} for surface {}", accessor.index(), mesh.name().unwrap_or(&mesh.index().to_string()) ));
                                                    let handle = asset_server.entry(m.clone());
                                                    if asset_server.get_state(&handle.clone().into_untyped_handle()) == Some(crate::asset::asset_state::AssetState::Unloaded) {
                                                        if let Err(e) = asset_server.transition_loading(&handle.clone().into_untyped_handle()) {
                                                            tracing::warn!("Failed to load: {e}");
                                                        }
                                                    }
                                                    handle
                                                });
                                            surface_builder.tangent_buffer = handle;
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
                    let mesh_name = mesh
                        .name()
                        .map(|name| name.to_string())
                        .unwrap_or(format!("Mesh {mesh_count}"));
                    let primitive_name = format!("{mesh_name} primitive {mesh_count}");
                    surfaces.push(engine::components::Mesh {
                        surface,
                        material: engine::components::Material {
                            albedo_factor: glam::Vec4::ONE,
                            albedo_texture: primitive.material().pbr_metallic_roughness().base_color_texture().map(|texture| {
                                 textures[texture.texture().index()].clone()
                            }),
                            alpha_mode: primitive.material().alpha_mode(),
                        },
                        bounding_box: bounding_box.unwrap_or(dare::render::components::bounding_box::BoundingBox::new(
                            glam::Vec3::from(primitive.bounding_box().min),
                            glam::Vec3::from(primitive.bounding_box().max),
                        )),
                        name: engine::components::Name(primitive_name),
                        transform: dare::physics::components::Transform {
                            scale,
                            rotation,
                            translation,
                        },
                    });
                    mesh_count += 1;
                }
                Ok(surfaces)
            })
            .flatten()
            .collect::<Vec<engine::components::Mesh>>();
        commands.spawn_batch(meshes.clone().into_iter());
        // same idea, but spawn it like +5 above
        Ok(())
    }
}
