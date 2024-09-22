use std::marker::PhantomData;
use std::sync::Arc;
use anyhow::Result;
use gltf;
use gltf::buffer::Source;
use dagal::allocators::Allocator;
use super::prelude as asset;
use bevy_ecs::prelude as becs;

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
    Required::No(GltfSemantics::UVs)
];

/// Handles gltf loading
pub struct GLTFLoader<A: Allocator + 'static> {
    /// Location of the .gltf file
    path: std::path::PathBuf,
    _allocator: PhantomData<A>,
}

impl<A: Allocator + 'static> GLTFLoader<A> {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self {
            path,
            _allocator: Default::default(),
        }
    }

    pub fn load(mut commands: becs::Commands, asset_manager: becs::Res<asset::AssetManager<A>>, path: std::path::PathBuf) -> Result<()> {
        let gltf: gltf::Gltf = gltf::Gltf::open(path.clone())?;
        let blob: Option<Arc<[u8]>> = gltf.blob.clone().map(|blob| {
            Arc::from(blob.into_boxed_slice())
        });
        let buffer_metadatas = gltf.buffers().map(|buffer| {
            let location: asset::MetaDataLocation = match buffer.source() {
                Source::Bin => {
                    if let Some(blob) = blob.clone() {
                        asset::MetaDataLocation::Memory(blob)
                    } else {
                        return Err::<_, anyhow::Error>(anyhow::anyhow!("Expected blob, got None"))
                    }
                }
                Source::Uri(uri) => {
                    if !uri.starts_with("data") {
                        let mut path = path.clone();
                        path.push(std::path::PathBuf::from(uri));
                        asset::MetaDataLocation::FilePath(path)
                    } else {
                        unimplemented!()
                    }
                }
            };
            Ok(asset::BufferMetaData::<A> {
                location,
                offset: 0,
                length: buffer.length(),
                stride: None,
                element_format: asset::Format::new(asset::ElementFormat::U8, 1),
                element_count: 0,
                _allocator: Default::default(),
            })
        }).collect::<Vec<Result<asset::BufferMetaData<A>>>>();
        let accessors_metadata = gltf.accessors().map(|accessor| {
            if accessor.sparse().is_some() {
                return Err::<_, anyhow::Error>(anyhow::anyhow!("Does not support sparse data"));
            } else if let Some(view) = accessor.view() {
                if let Ok(buffer_metadata) = buffer_metadatas.get(view.buffer().index()).unwrap() {
                    let mut buffer_metadata = buffer_metadata.clone();
                    buffer_metadata.length = view.length();
                    buffer_metadata.stride = view.stride();
                    buffer_metadata.offset = view.offset();
                    buffer_metadata.element_count = accessor.count();
                    buffer_metadata.element_format = asset::Format::new(asset::ElementFormat::from(
                        accessor.data_type()
                    ), accessor.dimensions().multiplicity());
                    Ok(buffer_metadata)
                } else {
                    Err(anyhow::anyhow!("No metadata found at {}", view.buffer().index()))
                }
            } else {
                unimplemented!()
            }
        }).collect::<Vec<Result<asset::BufferMetaData<A>>>>();
        let meshes = gltf.document.meshes().map(|mesh| {
            mesh.primitives().map(|primitive| {
                // retrieve all required prims
                //commands.spawn();
                let uv_indices: Vec<u32> = primitive.attributes().flat_map(|(attr, _)| match attr {
                        gltf::Semantic::TexCoords(i) => Some(i),
                        _ => None
                    }
                ).collect();
                for semantic in EXPECTED_SEMANTICS.iter() {
                    let is_required = match semantic {
                        Required::No(_) => false,
                        Required::Yes(_) => true,
                    };
                    let semantic = match semantic {
                        Required::No(semantic) => semantic,
                        Required::Yes(semantic) => semantic
                    };
                    let accessors: Vec<gltf::Accessor> = match semantic {
                        GltfSemantics::Index => match primitive.indices() {
                                None => if is_required {
                                    return Err(anyhow::anyhow!("Missing indices in primitive, got None"));
                                } else {
                                    vec![]
                                },
                                Some(accessor) => vec![accessor],
                            },
                        GltfSemantics::Accessor(semantic) => match primitive.get(semantic) {
                            None => if is_required {
                                return Err(anyhow::anyhow!("Missing accessor {:?}, got NULL", semantic));
                            } else {
                                vec![]
                            },
                            Some(accessor) => vec![accessor],
                        }
                        GltfSemantics::UVs => uv_indices.iter().map(|indices| {
                                primitive.get(&gltf::Semantic::TexCoords(*indices)).unwrap()
                            }).collect::<Vec<_>>()
                    };
                    match semantic {
                        GltfSemantics::Index => {
                        }
                        GltfSemantics::Accessor(_) => {}
                        GltfSemantics::UVs => {}
                    }
                }
                Ok::<(), anyhow::Error>(todo!())
            })
        });

        Ok(())
    }
}