use std::{mem, path, ptr};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use anyhow::Result;
use bytemuck::{cast_slice, Pod};
use futures::prelude::*;
use gltf::Gltf;
use gltf::texture::{MagFilter, WrappingMode};
use image::{ColorType, EncodableLayout, GenericImageView};
use rayon::prelude::*;
use tokio::sync::{RwLock, SemaphorePermit};
use tracing::warn;

use dagal::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use dagal::command::command_buffer::CmdBuffer;
use dagal::descriptor::bindless::bindless::ResourceInput;
use dagal::descriptor::DescriptorInfo::Image;
use dagal::descriptor::GPUResourceTable;
use dagal::pipelines::GraphicsPipeline;
use dagal::resource;
use dagal::resource::traits::{Nameable, Resource};
use dagal::util::free_list_allocator::Handle;
use dagal::util::ImmediateSubmit;

use crate::{assets, render, util};
use crate::render::SurfaceHandleBuilder;
use crate::util::handle;

/// Responsible for loading gltf assets
const CHUNK_MAX_SIZE: usize = 10usize.pow(9); // 1 GiB
const MAX_MEMORY_USAGE: usize = 4 * 10usize.pow(9); // Max amount of memory that can be used at any time to load meshes in

/// Struct which loads GLTF assets
#[derive(Debug)]
pub struct GltfLoader<'a> {
    immediate: &'a mut ImmediateSubmit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Semantic {
    Semantic(gltf::Semantic),
    Indices,
}

struct Chunk<T> {
    length: usize,
    chunks: Vec<Subchunk<T>>,
}

impl<T> Chunk<T> {
    fn chunkify(subchunks: Vec<Subchunk<T>>, size_limit: usize) -> Vec<Self> {
        let mut chunks: Vec<Chunk<T>> = Vec::new();
        let mut current_chunk: Chunk<T> = Chunk::default();
        let mut current_chunk_size: usize = 0;
        for mut subchunk in subchunks {
            if current_chunk_size >= size_limit {
                let mut old = Chunk::default();
                std::mem::swap(&mut old, &mut current_chunk);
                chunks.push(old);
                current_chunk_size = 0;
            }
            subchunk.offset = current_chunk_size;
            current_chunk_size += subchunk.size;
            current_chunk.length = current_chunk_size;
            current_chunk.chunks.push(subchunk);
        }
        chunks
    }
}

struct Subchunk<T> {
    handle: T,
    offset: usize,
    size: usize,
}

impl<T> Default for Chunk<T> {
    fn default() -> Self {
        Self {
            length: 0,
            chunks: vec![],
        }
    }
}

/// Represents a flatten node
struct FlattenNode<'a> {
    handle: gltf::Node<'a>,
    transform: glam::Mat4,
}

impl<'a> GltfLoader<'a> {
    pub fn new(immediate: &'a mut ImmediateSubmit) -> Self {
        Self { immediate }
    }

    /// Flattens node tree and applies transformations from the parent nodes
    fn flatten_node_tree<'b>(
        parent_transform: glam::Mat4,
        roots: Vec<gltf::Node<'b>>,
    ) -> Vec<FlattenNode<'b>> {
        roots
            .into_iter()
            .flat_map(|node| {
                let children: Vec<gltf::Node<'b>> = node.children().collect();
                let transform: glam::Mat4 =
                    parent_transform * glam::Mat4::from_cols_array_2d(&node.transform().matrix());
                let mut nodes: Vec<FlattenNode<'b>> = vec![FlattenNode {
                    handle: node.clone(),
                    transform,
                }];
                if !children.is_empty() {
                    nodes.append(&mut Self::flatten_node_tree(
                        transform,
                        node.children().collect(),
                    ))
                }
                nodes
            })
            .collect()
    }

    fn convert_and_cast<T, U>(slice: Vec<u8>) -> Vec<u8>
                              where
                                  T: Pod,
                                  U: Pod,
                                  T: Into<U>,
    {
        let from_slice: Vec<T> = cast_slice(&slice).to_vec();
        let to_slice: Vec<U> = from_slice.into_iter().map(|x| x.into()).collect();
        cast_slice(&to_slice).to_vec()
    }

    fn cast_bytes(
        from: gltf::accessor::DataType,
        to: gltf::accessor::DataType,
        slice: Vec<u8>,
    ) -> Vec<u8> {
        use gltf::accessor::DataType;
        match (from, to) {
            (DataType::F32, DataType::F32) => slice,
            (DataType::U16, DataType::U32) => Self::convert_and_cast::<u16, u32>(slice),
            (DataType::U32, DataType::U32) => slice,
            _ => panic!("{:?} -> {:?} not implemented.", from, to),
        }
    }

    fn load_buffer_source<'b>(
        blob: Option<&'b [u8]>,
        buffer: &'b mut Vec<u8>,
        source: gltf::buffer::Source,
        mut path: path::PathBuf,
    ) -> Result<&'b [u8]> {
        let blob: &'b [u8] = match source {
            gltf::buffer::Source::Bin => {
                blob.expect("GlTF file expected blob, but no blob provided!")
            }
            gltf::buffer::Source::Uri(uri) => {
                assert!(!uri.starts_with("data"));
                path.push(std::path::PathBuf::from(uri));
                std::fs::File::open(&path)
                    .unwrap()
                    .read_to_end(buffer)
                    .unwrap();
                &buffer[..]
            }
        };
        Ok(blob)
    }

    /// Loads an accessor's underlying data
    fn load_accessor(
        blob: Option<&[u8]>,
        accessor: gltf::Accessor,
        path: path::PathBuf,
    ) -> Vec<u8> {
        if let Some(view) = accessor.view() {
            let mut buffer: Vec<u8> = Vec::new();
            let blob =
                Self::load_buffer_source(blob, &mut buffer, view.buffer().source(), path).unwrap();
            let total_offset: usize = accessor.offset() + view.offset();
            let element_size: usize =
                accessor.data_type().size() * accessor.dimensions().multiplicity();
            let stride: usize = view.stride().unwrap_or(element_size);
            (0..accessor.count())
                .flat_map(|i| {
                    let start: usize = total_offset + stride * i;
                    let end: usize = start + element_size;
                    blob[start..end].iter().copied()
                })
                .collect()
        } else if let Some(sparse) = accessor.sparse() {
            unimplemented!()
        } else {
            panic!("Expected an accessor that is either sparse or non-sparse, but got none.")
        }
    }

    /// Loads an entire image
    pub fn load_image<'b>(
        blob: Option<&[u8]>,
        image: gltf::image::Image<'b>,
        mut path: path::PathBuf,
    ) -> Result<image::DynamicImage> {
        let mut buffer: Vec<u8> = Vec::new();
        match image.source() {
            gltf::image::Source::View { view, mime_type } => {
                let total_offset: usize = view.offset();
                let glob =
                    Self::load_buffer_source(blob, &mut buffer, view.buffer().source(), path)?;
                let glob: Vec<u8> = glob[total_offset..(total_offset + view.length())]
                    .chunks_exact(view.stride().unwrap_or(view.length()))
                    .flatten()
                    .copied()
                    .collect();
                let image = image::load_from_memory(&glob)?;
                Ok(image)
            }
            gltf::image::Source::Uri { uri, mime_type } => {
                assert!(!uri.starts_with("data"));
                path.push(path::PathBuf::from(uri));
                let file = image::io::Reader::open(path)?;
                let image = file.with_guessed_format()?.decode()?;
                match image.color() {
                    ColorType::L8 => {}
                    ColorType::La8 => {}
                    ColorType::Rgb8 => {}
                    ColorType::Rgba8 => {}
                    ColorType::L16 => {}
                    ColorType::La16 => {}
                    ColorType::Rgb16 => {}
                    ColorType::Rgba16 => {}
                    ColorType::Rgb32F => {}
                    ColorType::Rgba32F => {}
                    _ => unimplemented!(),
                }
                Ok(image)
            }
        }
    }

    /// Loads meshes from a gltf scene and uploads their buffer info onto the gpu
    pub async fn load_assets<A: Allocator + 'static>(
        &mut self,
        allocator: &'a mut ArcAllocator<A>,
        mut gpu_rt: GPUResourceTable<A>,
        path: path::PathBuf,
        pipeline: Arc<render::pipeline::Pipeline<GraphicsPipeline>>,
    ) -> Result<(Vec<Arc<render::Material<A>>>, Vec<assets::mesh::Mesh<A>>)> {
        let gltf: Arc<Gltf> = Arc::new(Gltf::open(path.clone())?);
        let parent_path = path.parent().unwrap().to_path_buf();
        let scene = gltf.default_scene().expect("No default scene found.");
        let mut child_indices: HashSet<usize> = HashSet::new();
        for node in gltf.nodes() {
            for child in node.children() {
                child_indices.insert(child.index());
            }
        }
        let root_nodes: Vec<_> = gltf
            .nodes()
            .filter(|node| !child_indices.contains(&node.index()))
            .collect();

        let nodes: Vec<FlattenNode> = Self::flatten_node_tree(glam::Mat4::IDENTITY, root_nodes);
        let mut samplers: Vec<handle::SamplerHandle<A>> = vec![handle::SamplerHandle::new(
            gpu_rt.new_sampler(ResourceInput::ResourceCI(
                resource::SamplerCreateInfo::FromCreateInfo {
                    device: gpu_rt.get_device().clone(),
                    create_info: vk::SamplerCreateInfo {
                        s_type: vk::StructureType::SAMPLER_CREATE_INFO,
                        p_next: ptr::null(),
                        flags: vk::SamplerCreateFlags::empty(),
                        mag_filter: Default::default(),
                        min_filter: Default::default(),
                        mipmap_mode: Default::default(),
                        address_mode_u: Default::default(),
                        address_mode_v: Default::default(),
                        address_mode_w: Default::default(),
                        mip_lod_bias: 0.0,
                        anisotropy_enable: 0,
                        max_anisotropy: 0.0,
                        compare_enable: 0,
                        compare_op: vk::CompareOp::NEVER,
                        min_lod: 0.0,
                        max_lod: 0.0,
                        border_color: Default::default(),
                        unnormalized_coordinates: 0,
                        _marker: Default::default(),
                    },
                    name: Some("default"),
                },
            ))?,
            gpu_rt.clone(),
        )];
        samplers.append(
            &mut gltf
                .samplers()
                .map(|sampler| {
                    let handle = gpu_rt.new_sampler(ResourceInput::ResourceCI(
                        resource::SamplerCreateInfo::FromCreateInfo {
                            device: gpu_rt.get_device().clone(),
                            create_info: vk::SamplerCreateInfo {
                                s_type: vk::StructureType::SAMPLER_CREATE_INFO,
                                p_next: ptr::null(),
                                flags: vk::SamplerCreateFlags::empty(),
                                mag_filter: sampler
                                    .mag_filter()
                                    .map(|mag_filter| match mag_filter {
                                        MagFilter::Nearest => vk::Filter::NEAREST,
                                        MagFilter::Linear => vk::Filter::LINEAR,
                                    })
                                    .unwrap_or_default(),
                                min_filter: sampler
                                    .mag_filter()
                                    .map(|min_filter| match min_filter {
                                        MagFilter::Nearest => vk::Filter::NEAREST,
                                        MagFilter::Linear => vk::Filter::LINEAR,
                                    })
                                    .unwrap_or_default(),
                                mipmap_mode: vk::SamplerMipmapMode::default(),
                                address_mode_u: match sampler.wrap_s() {
                                    WrappingMode::ClampToEdge => {
                                        vk::SamplerAddressMode::CLAMP_TO_EDGE
                                    }
                                    WrappingMode::MirroredRepeat => {
                                        vk::SamplerAddressMode::MIRRORED_REPEAT
                                    }
                                    WrappingMode::Repeat => vk::SamplerAddressMode::REPEAT,
                                },
                                address_mode_v: match sampler.wrap_t() {
                                    WrappingMode::ClampToEdge => {
                                        vk::SamplerAddressMode::CLAMP_TO_EDGE
                                    }
                                    WrappingMode::MirroredRepeat => {
                                        vk::SamplerAddressMode::MIRRORED_REPEAT
                                    }
                                    WrappingMode::Repeat => vk::SamplerAddressMode::REPEAT,
                                },
                                address_mode_w: Default::default(),
                                mip_lod_bias: 0.0,
                                anisotropy_enable: 0,
                                max_anisotropy: 0.0,
                                compare_enable: 0,
                                compare_op: vk::CompareOp::NEVER,
                                min_lod: 0.0,
                                max_lod: 0.0,
                                border_color: Default::default(),
                                unnormalized_coordinates: 0,
                                _marker: Default::default(),
                            },
                            name: sampler.name(),
                        },
                    ))?;
                    Ok::<handle::SamplerHandle<A>, anyhow::Error>(handle::SamplerHandle::new(
                        handle,
                        gpu_rt.clone(),
                    ))
                })
                .collect::<Result<Vec<handle::SamplerHandle<A>>>>()?,
        );
        println!("Loaded samplers");
        let images: Result<Vec<image::DynamicImage>> = (&gltf)
            .images()
            .collect::<Vec<gltf::image::Image>>()
            .into_par_iter()
            .map(|image| Self::load_image(gltf.blob.as_deref(), image, parent_path.clone()))
            .collect();
        let images: Vec<image::DynamicImage> = images.unwrap();
        #[derive(Debug)]
        struct GLTFImage<'a, A: Allocator = GPUAllocatorImpl> {
            gltf_image: gltf::image::Image<'a>,
            image: image::RgbaImage,
            vk_image: resource::Image<A>,
            vk_image_view: resource::ImageView,
        }

        impl<'a, A: Allocator> GLTFImage<'a, A> {
            pub fn decompose(self) -> (resource::Image<A>, resource::ImageView) {
                (self.vk_image, self.vk_image_view)
            }
        }

        let allocated_images: Vec<Chunk<GLTFImage<A>>> = {
            let mut allocated_images: Vec<Chunk<GLTFImage<A>>> = Vec::new();
            let mut current_chunk: Chunk<GLTFImage<A>> = Chunk::default();
            let mut current_size: usize = 0;
            for (gltf_image, image) in gltf.images().zip(&images) {
                let image = image.to_rgba8();
                let size_bytes = mem::size_of_val(image.as_bytes());
                let format = vk::Format::R8G8B8A8_SRGB;

                let mip_levels = image.height().ilog2().max(1);
                let device = gpu_rt.get_device().clone();
                let vk_image = resource::Image::new(resource::ImageCreateInfo::NewAllocated {
                    device: device.clone(),
                    allocator,
                    location: MemoryLocation::GpuOnly,
                    image_ci: vk::ImageCreateInfo {
                        s_type: vk::StructureType::IMAGE_CREATE_INFO,
                        p_next: ptr::null(),
                        flags: vk::ImageCreateFlags::empty(),
                        image_type: vk::ImageType::TYPE_2D,
                        format,
                        extent: vk::Extent3D {
                            width: image.width(),
                            height: image.height(),
                            depth: 1,
                        },
                        mip_levels,
                        array_layers: 1,
                        samples: vk::SampleCountFlags::TYPE_1,
                        tiling: vk::ImageTiling::LINEAR,
                        usage: vk::ImageUsageFlags::TRANSFER_DST
                            | vk::ImageUsageFlags::TRANSFER_SRC
                            | vk::ImageUsageFlags::SAMPLED,
                        sharing_mode: if device.get_used_queue_families().len() == 1 {
                            vk::SharingMode::EXCLUSIVE
                        } else {
                            vk::SharingMode::CONCURRENT
                        },
                        queue_family_index_count: device.get_used_queue_families().len() as u32,
                        p_queue_family_indices: device.get_used_queue_families().as_ptr(),
                        initial_layout: vk::ImageLayout::UNDEFINED,
                        _marker: Default::default(),
                    },
                    name: gltf_image.name(),
                })?;
                let vk_image_view =
                    resource::ImageView::new(resource::ImageViewCreateInfo::FromCreateInfo {
                        device: gpu_rt.get_device().clone(),
                        create_info: vk::ImageViewCreateInfo {
                            s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                            p_next: ptr::null(),
                            flags: vk::ImageViewCreateFlags::empty(),
                            image: vk_image.handle(),
                            view_type: vk::ImageViewType::TYPE_2D,
                            format,
                            components: vk::ComponentMapping::default(),
                            subresource_range: resource::image::Image::image_subresource_range(
                                vk::ImageAspectFlags::COLOR,
                            ),
                            _marker: Default::default(),
                        },
                    })?;
                current_chunk.chunks.push(Subchunk {
                    handle: GLTFImage {
                        gltf_image,
                        image,
                        vk_image,
                        vk_image_view,
                    },
                    offset: current_size,
                    size: size_bytes,
                });
                current_size += size_bytes;
                current_chunk.length = current_size;

                if current_size >= CHUNK_MAX_SIZE {
                    let mut old: Chunk<GLTFImage<A>> = Chunk::default();
                    mem::swap(&mut old, &mut current_chunk);
                    allocated_images.push(old);
                    current_size = 0;
                }
            }
            allocated_images.push(current_chunk);
            allocated_images
        };

        // Add staging buffers
        let mut images: Vec<(handle::ImageHandle<A>, handle::ImageViewHandle<A>)> =
            Vec::with_capacity(allocated_images.iter().map(|c| c.chunks.len()).sum());
        for (id, mut chunk) in allocated_images.into_iter().enumerate() {
            if chunk.chunks.is_empty() {
                continue;
            }
            println!("Loading {id}/{}", images.len());
            // make staging buffers + uploading
            let staging = resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                device: gpu_rt.get_device().clone(),
                allocator,
                size: chunk.length as vk::DeviceSize,
                memory_type: MemoryLocation::CpuToGpu,
                usage_flags: vk::BufferUsageFlags::TRANSFER_SRC,
            })?;
            chunk
                .chunks
                .par_iter()
                .map(|image| {
                    let ptr: *mut u8 = staging.mapped_ptr().unwrap().as_ptr() as *mut u8;
                    unsafe {
                        let slice =
                            std::slice::from_raw_parts_mut(ptr.add(image.offset), image.size);
                        slice.copy_from_slice(image.handle.image.as_bytes());
                    }
                })
                .collect::<()>();

            // Copy all data in parallel + submit commands
            self.immediate.submit(|ctx| {
                let cmd = ctx.cmd.handle();
                let queue = ctx.queue;
                let image_barrier: Vec<vk::ImageMemoryBarrier2> = chunk
                    .chunks
                    .iter()
                    .map(|image| vk::ImageMemoryBarrier2 {
                        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
                        p_next: ptr::null(),
                        src_stage_mask: vk::PipelineStageFlags2::TOP_OF_PIPE,
                        src_access_mask: vk::AccessFlags2::empty(),
                        dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                        dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                        old_layout: vk::ImageLayout::UNDEFINED,
                        new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        src_queue_family_index: queue.get_family_index(),
                        dst_queue_family_index: queue.get_family_index(),
                        image: image.handle.vk_image.handle(),
                        subresource_range: resource::Image::image_subresource_range(
                            vk::ImageAspectFlags::COLOR,
                        ),
                        _marker: Default::default(),
                    })
                    .collect();
                unsafe {
                    ctx.device.get_handle().cmd_pipeline_barrier2(
                        cmd,
                        &vk::DependencyInfo {
                            s_type: vk::StructureType::DEPENDENCY_INFO,
                            p_next: ptr::null(),
                            dependency_flags: vk::DependencyFlags::empty(),
                            memory_barrier_count: 0,
                            p_memory_barriers: ptr::null(),
                            buffer_memory_barrier_count: 0,
                            p_buffer_memory_barriers: ptr::null(),
                            image_memory_barrier_count: image_barrier.len() as u32,
                            p_image_memory_barriers: image_barrier.as_ptr(),
                            _marker: Default::default(),
                        },
                    );
                }
                println!("Copied from cpu to gpu");

                // copy
                chunk
                    .chunks
                    .iter()
                    .map(|image| {
                        let copy_info = vk::BufferImageCopy {
                            buffer_offset: image.offset as vk::DeviceAddress,
                            buffer_row_length: 0,
                            buffer_image_height: 0,
                            image_subresource: vk::ImageSubresourceLayers {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                mip_level: 0,
                                base_array_layer: 0,
                                layer_count: 1,
                            },
                            image_offset: Default::default(),
                            image_extent: image.handle.vk_image.extent(),
                        };
                        unsafe {
                            ctx.device.get_handle().cmd_copy_buffer_to_image(
                                ctx.cmd.handle(),
                                staging.handle(),
                                image.handle.vk_image.handle(),
                                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                &[copy_info],
                            );
                        }
                    })
                    .collect::<()>();
                println!("Copied staging to image");

                // transfer
                let image_barrier: Vec<vk::ImageMemoryBarrier2> = chunk
                    .chunks
                    .iter()
                    .map(|image| vk::ImageMemoryBarrier2 {
                        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
                        p_next: ptr::null(),
                        src_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                        dst_stage_mask: vk::PipelineStageFlags2::ALL_GRAPHICS,
                        dst_access_mask: vk::AccessFlags2::MEMORY_WRITE
                            | vk::AccessFlags2::MEMORY_READ,
                        old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        src_queue_family_index: queue.get_family_index(),
                        dst_queue_family_index: queue.get_family_index(),
                        image: image.handle.vk_image.handle(),
                        subresource_range: resource::Image::image_subresource_range(
                            vk::ImageAspectFlags::COLOR,
                        ),
                        _marker: Default::default(),
                    })
                    .collect();
                unsafe {
                    ctx.device.get_handle().cmd_pipeline_barrier2(
                        cmd,
                        &vk::DependencyInfo {
                            s_type: vk::StructureType::DEPENDENCY_INFO,
                            p_next: ptr::null(),
                            dependency_flags: vk::DependencyFlags::empty(),
                            memory_barrier_count: 0,
                            p_memory_barriers: ptr::null(),
                            buffer_memory_barrier_count: 0,
                            p_buffer_memory_barriers: ptr::null(),
                            image_memory_barrier_count: image_barrier.len() as u32,
                            p_image_memory_barriers: image_barrier.as_ptr(),
                            _marker: Default::default(),
                        },
                    );
                }
            });
            images.append(
                &mut chunk
                    .chunks
                    .into_iter()
                    .map(|image| {
                        let (image, image_view) = image.handle.decompose();
                        let (image_handle, image_view_handle) = gpu_rt.new_image(
                            ResourceInput::Resource(image),
                            ResourceInput::Resource(image_view),
                            vk::ImageLayout::GENERAL,
                        )?;
                        Ok((
                            handle::ImageHandle::new(image_handle, gpu_rt.clone()),
                            handle::ImageViewHandle::new(image_view_handle, gpu_rt.clone()),
                        ))
                    })
                    .collect::<Result<Vec<(handle::ImageHandle<A>, handle::ImageViewHandle<A>)>>>()?,
            );
        }
        println!("Loaded images {:?}", images.len());
        let textures: Vec<render::Texture<A>> = gltf
            .textures()
            .map(|gltf_texture| {
                let image = images.get(gltf_texture.source().index()).unwrap().clone();
                render::Texture::from_handles(
                    image.0,
                    image.1,
                    samplers
                        .get(gltf_texture.sampler().index().map(|id| id + 1).unwrap_or(0))
                        .unwrap()
                        .clone(),
                )
            })
            .collect();
        let mut materials: Vec<Arc<render::Material<A>>> = vec![{
            let mut material = render::Material::new(
                allocator,
                pipeline.clone(),
                glam::Vec4::from([1.0, 1.0, 1.0, 1.0]),
                None,
                None,
                String::from("default"),
                gpu_rt.get_device().clone(),
            )
                .unwrap();
            material.upload_material(self.immediate, allocator).unwrap();
            Arc::new(material)
        }];
        materials.append(
            &mut gltf
                .materials()
                .filter_map(|gltf_material| {
                    if gltf_material.index().is_none() {
                        None
                    } else {
                        let mut material = render::Material::new(
                            allocator,
                            pipeline.clone(),
                            glam::Vec4::from(
                                gltf_material.pbr_metallic_roughness().base_color_factor(),
                            ),
                            gltf_material
                                .pbr_metallic_roughness()
                                .base_color_texture()
                                .and_then(|tex| textures.get(tex.texture().index()).cloned()),
                            gltf_material
                                .normal_texture()
                                .and_then(|tex| textures.get(tex.texture().index()).cloned()),
                            gltf_material
                                .name()
                                .map(|n| n.to_string())
                                .unwrap_or(format!(
                                    "{}_material",
                                    gltf_material.index().unwrap_or(0)
                                )),
                            gpu_rt.get_device().clone(),
                        )
                            .unwrap();
                        material.upload_material(self.immediate, allocator).unwrap();
                        Some(Arc::new(material))
                    }
                })
                .collect(),
        );
        println!("Loaded materials");

        // Load all mesh resources
        struct AccessorAllocation<A: Allocator = GPUAllocatorImpl> {
            data: Option<Vec<u8>>,
            buffer: Option<resource::Buffer<A>>,
            semantic: Semantic,
            mesh_index: usize,
            primitive_index: usize,
            accessor_index: usize,
        }
        impl<A: Allocator> AccessorAllocation<A> {
            pub fn new(
                primitive: &gltf::Primitive,
                semantic: Semantic,
                mesh_index: usize,
            ) -> Option<Self> {
                let accessor: Option<gltf::Accessor> = match &semantic {
                    Semantic::Semantic(semantic) => primitive.get(semantic),
                    Semantic::Indices => primitive.indices(),
                };

                if let Some(accessor) = accessor {
                    let accessor_index: usize = accessor.index();
                    Some(AccessorAllocation {
                        data: None,
                        buffer: None,
                        semantic,
                        mesh_index,
                        primitive_index: primitive.index(),
                        accessor_index,
                    })
                } else {
                    None
                }
            }
        }
        let meshes: Vec<gltf::Mesh> = gltf.meshes().collect();
        let accessors: Vec<gltf::Accessor> = gltf.accessors().collect();
        let mesh_allocations: Vec<Subchunk<AccessorAllocation<A>>> = meshes.into_iter().flat_map(|mesh| {
            mesh.primitives().filter_map(|primitive| {
                if primitive.indices().is_none() || primitive.get(&gltf::Semantic::Positions).is_none() {
                    return None;
                }
                let mut accessor_allocations: Vec<Subchunk<AccessorAllocation<A>>> = Vec::new();
                for semantic in [Semantic::Indices, Semantic::Semantic(gltf::Semantic::Positions), Semantic::Semantic(gltf::Semantic::Normals), Semantic::Semantic(gltf::Semantic::TexCoords(0))] {
                    if let Some(accessor_allocation) = AccessorAllocation::new(&primitive, semantic, mesh.index()) {
                        let accessor = accessors.get(accessor_allocation.accessor_index).unwrap();
                        let size: usize = match accessor.view() {
                            None => {
                                warn!("There is not a full implementation of sparse accessors yet");
                                accessor.sparse().as_ref().unwrap().indices().view().buffer().length() + accessor.sparse().as_ref().unwrap().values().view().buffer().length()
                            }
                            Some(view) => {
                                view.buffer().length()
                            }
                        };
                        accessor_allocations.push(
                            Subchunk {
                                handle: accessor_allocation,
                                offset: 0,
                                // get the size that would be for loading in the gltf buffers
                                size,
                            }
                        );
                    }
                }
                Some(accessor_allocations)
            }).flatten().collect::<Vec<Subchunk<AccessorAllocation<A>>>>()
        }).collect::<Vec<Subchunk<AccessorAllocation<A>>>>();
        // Massively multi-thread mesh loading
        let mesh_allocation_size = mesh_allocations.len();
        let accessors: Vec<Subchunk<AccessorAllocation<A>>> = {
            let finished_pool: Arc<RwLock<Vec<Subchunk<AccessorAllocation<A>>>>> =
                Arc::new(RwLock::new(Default::default()));
            let cpu_memory_pool: Arc<tokio::sync::Semaphore> =
                Arc::new(tokio::sync::Semaphore::new(MAX_MEMORY_USAGE));
            let remaining_tasks = Arc::new(AtomicUsize::new(mesh_allocation_size));
            let (chunk_found_sender, mut reciever) = tokio::sync::mpsc::channel::<(
                resource::Buffer<A>,
                Vec<Subchunk<AccessorAllocation<A>>>,
            )>(1);
            let mut tasks: Vec<tokio::task::JoinHandle<()>> = mesh_allocations
                .into_iter()
                .enumerate()
                .map(|(id, mut subchunk)| {
                    let cpu_memory_pool: Arc<tokio::sync::Semaphore> = cpu_memory_pool.clone();
                    let chunk_found_sender = chunk_found_sender.clone();
                    let gltf: Arc<Gltf> = gltf.clone();
                    let parent_path = parent_path.clone();
                    let allocator = allocator.clone();
                    let device = gpu_rt.get_device().clone();
                    let finished_pool = finished_pool.clone();
                    let remaining_tasks: Arc<AtomicUsize> = remaining_tasks.clone();
                    tokio::task::spawn(async move {
                        println!("Loading {id}/{} primitive", mesh_allocation_size - 1);
                        // process data
                        let initial_permit = cpu_memory_pool
                            .acquire_many(subchunk.size as u32)
                            .await
                            .unwrap();
                        let accessor = gltf
                            .accessors()
                            .nth(subchunk.handle.accessor_index)
                            .unwrap();
                        let data: Vec<u8> = Self::load_accessor(
                            gltf.blob.as_deref(),
                            accessor.clone(),
                            parent_path,
                        );
                        let data: Vec<u8> = Self::cast_bytes(
                            accessor.data_type(),
                            match subchunk.handle.semantic {
                                Semantic::Semantic(_) => gltf::accessor::DataType::F32,
                                Semantic::Indices => gltf::accessor::DataType::U32,
                            },
                            data,
                        );
                        subchunk.handle.data = Some(data);
                        // adjust semaphores + update size
                        let data_size: usize =
                            mem::size_of_val(subchunk.handle.data.as_deref().unwrap());
                        let new_permit: Option<SemaphorePermit> =
                            match data_size.cmp(&subchunk.size) {
                                Ordering::Less => {
                                    cpu_memory_pool.add_permits(subchunk.size - data_size);
                                    None
                                }
                                Ordering::Equal => None,
                                Ordering::Greater => Some(
                                    cpu_memory_pool
                                        .acquire_many((data_size - subchunk.size) as u32)
                                        .await
                                        .unwrap(),
                                ),
                            };
                        subchunk.size = data_size;

                        // create buffer
                        let mut allocator = allocator.clone();
                        let buffer =
                            resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                                device: device.clone(),
                                allocator: &mut allocator,
                                size: subchunk.size as vk::DeviceAddress,
                                memory_type: MemoryLocation::GpuOnly,
                                usage_flags: vk::BufferUsageFlags::TRANSFER_DST
                                    | vk::BufferUsageFlags::STORAGE_BUFFER
                                    | vk::BufferUsageFlags::VERTEX_BUFFER
                                    | vk::BufferUsageFlags::INDEX_BUFFER
                                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                            })
                                .unwrap();
                        subchunk.handle.buffer = Some(buffer);
                        // clean up
                        initial_permit.forget();
                        if let Some(permit) = new_permit {
                            permit.forget();
                        }

                        // check if finished pool has sufficient size
                        let mut finished_guard = finished_pool.write().await;
                        finished_guard.push(subchunk);
                        let finished_pool_size: usize =
                            finished_guard.iter().map(|sc| sc.size).sum();
                        let remaining_tasks = remaining_tasks.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                        if finished_pool_size >= CHUNK_MAX_SIZE || remaining_tasks <= 1 {
                            let mut chunk_length: usize = 0;
                            // set offsets
                            let mut finished_subchunks: Vec<Subchunk<AccessorAllocation<A>>> =
                                finished_guard
                                    .drain(..)
                                    .map(|mut subchunk| {
                                        subchunk.offset = chunk_length;
                                        chunk_length += subchunk.size;
                                        subchunk
                                    })
                                    .collect();
                            drop(finished_guard);
                            // make staging
                            let staging_buffer =
                                resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                                    device,
                                    allocator: &mut allocator,
                                    size: finished_pool_size as vk::DeviceSize,
                                    memory_type: MemoryLocation::CpuToGpu,
                                    usage_flags: vk::BufferUsageFlags::TRANSFER_SRC,
                                }).unwrap();

                            // Use unsafe code to allow parallel writes
                            finished_subchunks.par_iter_mut().for_each(|subchunk| {
                                let dst_ptr: *mut u8 = staging_buffer.mapped_ptr().unwrap().as_ptr() as *mut u8;
                                unsafe {
                                    ptr::copy_nonoverlapping(subchunk.handle.data.as_deref().unwrap().as_ptr() as *mut u8, dst_ptr.add(subchunk.offset), subchunk.size);
                                    drop(subchunk.handle.data.take());
                                    cpu_memory_pool.add_permits(subchunk.size);
                                }
                            });

                            chunk_found_sender
                                .send((staging_buffer, finished_subchunks))
                                .await
                                .unwrap();
                        }
                    })
                })
                .collect();
            drop(chunk_found_sender);
            assert_eq!(tasks.len(), mesh_allocation_size);

            let mut total_subchunks: Vec<Subchunk<AccessorAllocation<A>>> = Vec::new();
            let mut join_handles_stream = stream::FuturesUnordered::new();
            for handle in tasks {
                join_handles_stream.push(handle);
            }


            loop {
                tokio::select! {
                    Some((staging_buffer, mut subchunks)) = reciever.recv() => {
                       self.immediate.submit(|ctx| {
                            for subchunk in subchunks.iter() {
                                unsafe {
                                    ctx.device.get_handle().cmd_copy_buffer(ctx.cmd.handle(), staging_buffer.handle(), subchunk.handle.buffer.as_ref().unwrap().handle(), &[
                                        vk::BufferCopy {
                                            dst_offset: 0,
                                            src_offset: subchunk.offset as vk::DeviceAddress,
                                            size: subchunk.size as vk::DeviceAddress,
                                        }
                                    ]);
                                }
                            }
                            println!("Transferred {} tasks to the GPU, remaining: {}", subchunks.len(), )
                            total_subchunks.append(&mut subchunks);
                        });
                    },
                    Some(_) = join_handles_stream.next() => {
                        println!("A task completed, remaining: {}", remaining_tasks.fetch_add(0, std::sync::atomic::Ordering::SeqCst));
                    },
                    else => {
                        break;
                    },
                }
            }
            total_subchunks
        };
        assert_eq!(accessors.len(), mesh_allocation_size);

        let mut accessors: HashMap<(usize, usize, usize), Handle<resource::Buffer<A>>> = accessors
            .into_iter()
            .map(|accessor| {
                (
                    (
                        accessor.handle.mesh_index,
                        accessor.handle.primitive_index,
                        accessor.handle.accessor_index,
                    ),
                    gpu_rt
                        .new_buffer(ResourceInput::Resource(accessor.handle.buffer.unwrap()))
                        .unwrap(),
                )
            })
            .collect();

        let mut mesh_surfaces: HashMap<usize, Vec<Arc<render::Surface<A>>>> = HashMap::new();
        let meshes: Vec<assets::mesh::Mesh<A>> = nodes
            .into_iter()
            .enumerate()
            .filter_map(|(mesh_id, node)| {
                if let Some(mesh) = node.handle.mesh() {
                    let primitives: Vec<Arc<render::Surface<A>>> = mesh_surfaces
                        .entry(mesh.index())
                        .or_insert_with(|| {
                            mesh.primitives()
                                .enumerate()
                                .filter_map(|(primitive_id, primitive)| {
                                    if primitive.indices().is_none()
                                        || primitive.get(&gltf::Semantic::Positions).is_none()
                                    {
                                        return None;
                                    }
                                    let indices = primitive.indices().unwrap();
                                    assert_eq!(indices.dimensions(), gltf::accessor::Dimensions::Scalar);
                                    let positions =
                                        primitive.get(&gltf::Semantic::Positions).unwrap();
                                    assert_eq!(positions.dimensions(), gltf::accessor::Dimensions::Vec3);
                                    let normal = primitive.get(&gltf::Semantic::Normals);
                                    let uv = primitive.get(&gltf::Semantic::TexCoords(0));
                                    Some(Arc::new({
                                        let mut surface =
                                            render::Surface::from_handles(SurfaceHandleBuilder {
                                                gpu_rt: gpu_rt.clone(),
                                                allocator,
                                                material: materials
                                                    .get(
                                                        primitive
                                                            .material()
                                                            .index()
                                                            .map(|id| id + 1)
                                                            .unwrap_or(0),
                                                    )
                                                    .unwrap()
                                                    .clone(),
                                                indices: accessors
                                                    .remove(&(
                                                        mesh.index(), primitive.index(), indices.index()
                                                    ))
                                                    .unwrap(),
                                                positions: accessors
                                                    .remove(&(
                                                        mesh.index(),
                                                        primitive.index(),
                                                        positions.index(),
                                                    ))
                                                    .unwrap()
                                                    .clone(),
                                                normals: None,
                                                uv: uv.map(|accessor| {
                                                    accessors
                                                        .remove(&(
                                                            mesh.index(),
                                                            primitive.index(),
                                                            accessor.index(),
                                                        ))
                                                        .unwrap()
                                                }),
                                                total_indices: indices.count() as u32,
                                                first_index: 0,
                                                name: format!(
                                                    "primitive_{}_{}",
                                                    mesh.name()
                                                        .unwrap_or(mesh_id.to_string().as_str()),
                                                    primitive_id
                                                )
                                                    .as_str(),
                                            })
                                                .unwrap();
                                        surface
                                            .upload(self.immediate, allocator, node.transform)
                                            .unwrap();
                                        surface
                                    }))
                                })
                                .collect::<Vec<Arc<render::Surface<A>>>>()
                        })
                        .clone();

                    Some(assets::mesh::Mesh::new(
                        mesh.name().map(|n| n.to_string()),
                        node.transform.w_axis.truncate(),
                        glam::Vec3::new(
                            node.transform.x_axis.length(),
                            node.transform.y_axis.length(),
                            node.transform.z_axis.length(),
                        ),
                        glam::Quat::from_mat4(&node.transform),
                        primitives,
                    ))
                } else {
                    None
                }
            })
            .collect();
        println!("Loaded meshes");
        Ok((materials, meshes))
    }
}
