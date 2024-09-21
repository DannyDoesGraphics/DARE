use std::marker::PhantomData;
use std::sync::Arc;
use anyhow::Result;
use gltf;
use gltf::buffer::Source;
use dagal::allocators::Allocator;
use super::prelude as asset;

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

    pub fn load(render_world: &mut bevy_ecs::world::World, path: std::path::PathBuf) -> Result<()> {
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
        let accessors = gltf.accessors().map(|accessor| {
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

            })
        });

        Ok(())
    }
}