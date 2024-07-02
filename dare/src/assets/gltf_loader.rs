use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::os::windows::prelude::MetadataExt;
use std::sync::{Arc, Mutex};
use std::{mem, path, ptr};

use anyhow::Result;
use bytemuck::{cast_slice, Pod};
use futures::prelude::*;
use gltf::image::Source;
use gltf::texture::{MagFilter, WrappingMode};
use gltf::Gltf;

use dagal::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use dagal::command::command_buffer::CmdBuffer;
use dagal::descriptor::bindless::bindless::ResourceInput;
use dagal::descriptor::GPUResourceTable;
use dagal::pipelines::GraphicsPipeline;
use dagal::resource;
use dagal::resource::traits::Resource;
use dagal::util::free_list_allocator::Handle;
use dagal::util::ImmediateSubmit;

use crate::render::SurfaceHandleBuilder;
use crate::util::handle;
use crate::{assets, render};

/// Responsible for loading gltf assets
const CPU_MAX_MEMORY_USAGE: usize = 4 * 10usize.pow(9); // Max amount of memory that can be used at any time to load meshes in
const GPU_MAX_MEMORY_USAGE: usize = 2 * 10usize.pow(9); // Max amount of memory that can be used during transfer

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
        } else if accessor.sparse().is_some() {
            unimplemented!()
        } else {
            panic!("Expected an accessor that is either sparse or non-sparse, but got none.")
        }
    }

    /// Loads an entire image
    pub async fn load_image<'b>(
        blob: Option<&[u8]>,
        image: &gltf::image::Image<'b>,
        mut path: path::PathBuf,
    ) -> Result<image::DynamicImage> {
        let mut buffer: Vec<u8> = Vec::new();
        let buf: Vec<u8> = match image.source() {
            Source::View { view, .. } => {
                let total_offset: usize = view.offset();
                let glob =
                    Self::load_buffer_source(blob, &mut buffer, view.buffer().source(), path)?;
                let glob: Vec<u8> = glob[total_offset..(total_offset + view.length())]
                    .chunks_exact(view.stride().unwrap_or(view.length()))
                    .flatten()
                    .copied()
                    .collect();
                glob
            }
            Source::Uri { uri, .. } => {
                assert!(!uri.starts_with("data"));
                path.push(path::PathBuf::from(uri));
                std::fs::read(path)?
            }
        };
        Ok(image::load_from_memory(&buf)?)
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
        gltf.default_scene().expect("No default scene found.");
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

        enum AssetFinished<A: Allocator = GPUAllocatorImpl> {
            Image {
                index: usize,
                image: resource::Image<A>,
                image_view: resource::ImageView,
            },
            Buffer {
                index: AccessorIndex,
                buffer: resource::Buffer<A>,
            },
        }
        enum AssetLoading {
            Accessor {
                index: usize,
                primitive: usize,
                mesh_index: usize,
                semantic: Semantic,
            },
            Image {
                index: usize,
            },
        }

        struct AssetToLoad {
            size: usize,
            asset: AssetLoading,
        }

        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        struct AccessorIndex {
            mesh_index: usize,
            primitive_index: usize,
            accessor_index: usize,
        }

        let assets_to_load: Vec<AssetToLoad> = {
            let mut images: Vec<AssetToLoad> = gltf
                .images()
                .map(|gltf_image| {
                    let size: usize = match gltf_image.source() {
                        Source::View { view, .. } => view.buffer().length(),
                        Source::Uri { uri, .. } => {
                            assert!(!uri.starts_with("data"));
                            let mut parent_path: path::PathBuf = parent_path.clone();
                            parent_path.push(uri);
                            parent_path.metadata().unwrap().file_size() as usize
                        }
                    };
                    AssetToLoad {
                        size,
                        asset: AssetLoading::Image {
                            index: gltf_image.index(),
                        },
                    }
                })
                .collect::<Vec<AssetToLoad>>();
            let accessors: Vec<AssetToLoad> = gltf
                .meshes()
                .flat_map(|gltf_mesh| {
                    gltf_mesh
                        .primitives()
                        .filter_map(|gltf_primitive| {
                            if gltf_primitive.indices().is_none()
                                || gltf_primitive.get(&gltf::Semantic::Positions).is_none()
                            {
                                return None;
                            }
                            Some(
                                [
                                    Semantic::Indices,
                                    Semantic::Semantic(gltf::Semantic::Positions),
                                    Semantic::Semantic(gltf::Semantic::Normals),
                                    Semantic::Semantic(gltf::Semantic::TexCoords(0)),
                                ]
                                .into_iter()
                                .filter_map(|semantic| {
                                    let accessor: gltf::Accessor = match &semantic {
                                        Semantic::Semantic(semantic) => {
                                            gltf_primitive.get(semantic)?
                                        }
                                        Semantic::Indices => gltf_primitive.indices()?,
                                    };
                                    let size: usize = match accessor.view() {
                                        None => unimplemented!(),
                                        Some(view) => match view.buffer().source() {
                                            gltf::buffer::Source::Bin => 0,
                                            gltf::buffer::Source::Uri(uri) => {
                                                let mut parent_path = parent_path.clone();
                                                parent_path.push(uri);
                                                parent_path.metadata().unwrap().len() as usize
                                            }
                                        },
                                    };
                                    Some(AssetToLoad {
                                        size,
                                        asset: AssetLoading::Accessor {
                                            index: accessor.index(),
                                            primitive: gltf_primitive.index(),
                                            mesh_index: gltf_mesh.index(),
                                            semantic,
                                        },
                                    })
                                })
                                .collect::<Vec<AssetToLoad>>(),
                            )
                        })
                        .flatten()
                        .collect::<Vec<AssetToLoad>>()
                })
                .collect::<Vec<AssetToLoad>>();
            images.extend(accessors);
            images
        };

        let (images, mut accessors) = {
            let cpu_memory_pool: Arc<tokio::sync::Semaphore> =
                Arc::new(tokio::sync::Semaphore::new(CPU_MAX_MEMORY_USAGE));
            let gpu_memory_pool: Arc<tokio::sync::Semaphore> =
                Arc::new(tokio::sync::Semaphore::new(GPU_MAX_MEMORY_USAGE));
            let vk_queue_family_index: u32 = self.immediate.get_queue().get_family_index();
            let vk_queue: Arc<Mutex<dagal::device::Queue>> =
                Arc::new(Mutex::new(*self.immediate.get_queue()));
            let tasks: Vec<tokio::task::JoinHandle<Result<AssetFinished<A>>>> = assets_to_load
                .into_iter()
                .map(|asset_to_load| {
                    let gltf = gltf.clone();
                    let vk_queue = vk_queue.clone();
                    let cpu_memory_pool = cpu_memory_pool.clone();
                    let gpu_memory_pool = gpu_memory_pool.clone();
                    let gpu_rt = gpu_rt.clone();
                    let mut allocator = allocator.clone();
                    let parent_path = parent_path.clone();
                    tokio::spawn(async move {
                        let raw_size: usize = asset_to_load.size;
                        let mut image_extents: Option<vk::Extent3D> = None;
                        let data: Vec<u8> = match &asset_to_load.asset {
                            AssetLoading::Accessor {
                                index, semantic, ..
                            } => {
                                let accessor: gltf::Accessor =
                                    gltf.accessors().nth(*index).unwrap();
                                let data: Vec<u8> = Self::load_accessor(
                                    gltf.blob.as_deref(),
                                    accessor.clone(),
                                    parent_path,
                                );
                                let data: Vec<u8> = Self::cast_bytes(
                                    accessor.data_type(),
                                    match semantic {
                                        Semantic::Semantic(_) => gltf::accessor::DataType::F32,
                                        Semantic::Indices => gltf::accessor::DataType::U32,
                                    },
                                    data,
                                );
                                data
                            }
                            AssetLoading::Image { index } => {
                                let image: gltf::Image = gltf.images().nth(*index).unwrap();
                                let image =
                                    Self::load_image(gltf.blob.as_deref(), &image, parent_path)
                                        .await?;
                                let image = image.to_rgba8();
                                image_extents = Some(vk::Extent3D {
                                    width: image.width(),
                                    height: image.height(),
                                    depth: 1,
                                });
                                image.into_raw()
                            }
                        };
                        let data_size: usize = mem::size_of_val(data.as_slice());
                        {
                            let data_diff: usize = data_size.abs_diff(raw_size);
                            match data_size.cmp(&raw_size) {
                                Ordering::Less => {
                                    cpu_memory_pool.add_permits(data_diff);
                                }
                                Ordering::Equal => {}
                                Ordering::Greater => {
                                    cpu_memory_pool
                                        .acquire_many(data_diff as u32)
                                        .await?
                                        .forget();
                                }
                            };
                        }
                        gpu_memory_pool
                            .acquire_many(data_size as u32)
                            .await?
                            .forget();
                        let staging_buffer =
                            resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                                device: gpu_rt.get_device().clone(),
                                allocator: &mut allocator,
                                size: data_size as vk::DeviceSize,
                                memory_type: MemoryLocation::CpuToGpu,
                                usage_flags: vk::BufferUsageFlags::TRANSFER_SRC,
                            })?;
                        unsafe {
                            (staging_buffer.mapped_ptr().unwrap().as_ptr() as *mut u8)
                                .copy_from_nonoverlapping(data.as_ptr(), data_size);
                            drop(data);
                            cpu_memory_pool.add_permits(data_size);
                        }

                        let resource: AssetFinished<A> = match &asset_to_load.asset {
                            AssetLoading::Accessor {
                                index: accessor_index,
                                primitive: primitive_index,
                                mesh_index,
                                ..
                            } => {
                                let buffer = resource::Buffer::new(
                                    resource::BufferCreateInfo::NewEmptyBuffer {
                                        device: gpu_rt.get_device().clone(),
                                        allocator: &mut allocator,
                                        size: data_size as vk::DeviceSize,
                                        memory_type: MemoryLocation::GpuOnly,
                                        usage_flags: vk::BufferUsageFlags::TRANSFER_DST
                                            | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                                            | vk::BufferUsageFlags::STORAGE_BUFFER
                                            | vk::BufferUsageFlags::INDEX_BUFFER
                                            | vk::BufferUsageFlags::VERTEX_BUFFER,
                                    },
                                )?;
                                AssetFinished::Buffer {
                                    index: AccessorIndex {
                                        mesh_index: *mesh_index,
                                        primitive_index: *primitive_index,
                                        accessor_index: *accessor_index,
                                    },
                                    buffer,
                                }
                            }
                            AssetLoading::Image { index } => {
                                let extent = image_extents.unwrap();
                                let vk_image = resource::Image::<A>::new(
                                    resource::ImageCreateInfo::NewAllocated {
                                        device: gpu_rt.get_device().clone(),
                                        allocator: &mut allocator,
                                        location: MemoryLocation::GpuOnly,
                                        image_ci: vk::ImageCreateInfo {
                                            s_type: vk::StructureType::IMAGE_CREATE_INFO,
                                            p_next: ptr::null(),
                                            flags: vk::ImageCreateFlags::empty(),
                                            image_type: vk::ImageType::TYPE_2D,
                                            format: vk::Format::R8G8B8A8_SRGB,
                                            extent,
                                            mip_levels: extent.height.ilog2().max(1),
                                            array_layers: 1,
                                            samples: vk::SampleCountFlags::TYPE_1,
                                            tiling: vk::ImageTiling::LINEAR,
                                            usage: vk::ImageUsageFlags::TRANSFER_DST
                                                | vk::ImageUsageFlags::TRANSFER_SRC
                                                | vk::ImageUsageFlags::SAMPLED,
                                            sharing_mode: vk::SharingMode::EXCLUSIVE,
                                            queue_family_index_count: 1,
                                            p_queue_family_indices: &vk_queue_family_index,
                                            initial_layout: vk::ImageLayout::UNDEFINED,
                                            _marker: Default::default(),
                                        },
                                        name: None,
                                    },
                                )?;
                                let vk_image_view = resource::ImageView::new(
                                    resource::ImageViewCreateInfo::FromCreateInfo {
                                        device: gpu_rt.get_device().clone(),
                                        create_info: vk::ImageViewCreateInfo {
                                            s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                                            p_next: ptr::null(),
                                            flags: vk::ImageViewCreateFlags::empty(),
                                            image: vk_image.handle(),
                                            view_type: vk::ImageViewType::TYPE_2D,
                                            format: vk::Format::R8G8B8A8_SRGB,
                                            components: vk::ComponentMapping::default(),
                                            subresource_range:
                                                resource::image::Image::image_subresource_range(
                                                    vk::ImageAspectFlags::COLOR,
                                                ),
                                            _marker: Default::default(),
                                        },
                                    },
                                )?;
                                AssetFinished::Image {
                                    index: *index,
                                    image: vk_image,
                                    image_view: vk_image_view,
                                }
                            }
                        };
                        let device = gpu_rt.get_device().clone();
                        let vk_fence: dagal::sync::Fence =
                            dagal::sync::Fence::new(device.clone(), vk::FenceCreateFlags::empty())?;
                        let mut _command_buffer: Option<dagal::command::CommandBuffer> = None;
                        let mut _command_pool: Option<dagal::command::CommandPool> = None;
                        {
                            let vk_guard = vk_queue.lock().unwrap();
                            _command_pool = Some(dagal::command::CommandPool::new(
                                device.clone(),
                                &vk_guard,
                                vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                            )?);
                            let vk_command_buffer: dagal::command::CommandBuffer =
                                _command_pool.as_ref().unwrap().allocate(1)?.pop().unwrap();
                            vk_command_buffer.reset(vk::CommandBufferResetFlags::empty())?;

                            let vk_command_buffer = vk_command_buffer
                                .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
                                .unwrap();

                            match &resource {
                                AssetFinished::Image { image, .. } => unsafe {
                                    device.get_handle().cmd_pipeline_barrier2(
                                        vk_command_buffer.handle(),
                                        &vk::DependencyInfo {
                                            s_type: vk::StructureType::DEPENDENCY_INFO,
                                            p_next: ptr::null(),
                                            dependency_flags: vk::DependencyFlags::empty(),
                                            memory_barrier_count: 0,
                                            p_memory_barriers: ptr::null(),
                                            buffer_memory_barrier_count: 0,
                                            p_buffer_memory_barriers: ptr::null(),
                                            image_memory_barrier_count: 1,
                                            p_image_memory_barriers: &vk::ImageMemoryBarrier2 {
                                                s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
                                                p_next: ptr::null(),
                                                src_stage_mask:
                                                    vk::PipelineStageFlags2::TOP_OF_PIPE,
                                                src_access_mask: vk::AccessFlags2::empty(),
                                                dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                                                dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                                                old_layout: vk::ImageLayout::UNDEFINED,
                                                new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                                src_queue_family_index: vk_queue_family_index,
                                                dst_queue_family_index: vk_queue_family_index,
                                                image: image.handle(),
                                                subresource_range:
                                                    resource::Image::image_subresource_range(
                                                        vk::ImageAspectFlags::COLOR,
                                                    ),
                                                _marker: Default::default(),
                                            },
                                            _marker: Default::default(),
                                        },
                                    );

                                    device.get_handle().cmd_copy_buffer_to_image(
                                        vk_command_buffer.handle(),
                                        staging_buffer.handle(),
                                        image.handle(),
                                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                        &[vk::BufferImageCopy {
                                            buffer_offset: 0,
                                            buffer_row_length: 0,
                                            buffer_image_height: 0,
                                            image_subresource: vk::ImageSubresourceLayers {
                                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                                mip_level: 0,
                                                base_array_layer: 0,
                                                layer_count: 1,
                                            },
                                            image_offset: Default::default(),
                                            image_extent: image.extent(),
                                        }],
                                    );

                                    device.get_handle().cmd_pipeline_barrier2(
                                        vk_command_buffer.handle(),
                                        &vk::DependencyInfo {
                                            s_type: vk::StructureType::DEPENDENCY_INFO,
                                            p_next: ptr::null(),
                                            dependency_flags: vk::DependencyFlags::empty(),
                                            memory_barrier_count: 0,
                                            p_memory_barriers: ptr::null(),
                                            buffer_memory_barrier_count: 0,
                                            p_buffer_memory_barriers: ptr::null(),
                                            image_memory_barrier_count: 1,
                                            p_image_memory_barriers: &vk::ImageMemoryBarrier2 {
                                                s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
                                                p_next: ptr::null(),
                                                src_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                                                src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                                                dst_stage_mask:
                                                    vk::PipelineStageFlags2::ALL_GRAPHICS,
                                                dst_access_mask: vk::AccessFlags2::MEMORY_WRITE
                                                    | vk::AccessFlags2::MEMORY_READ,
                                                old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                                new_layout:
                                                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                                                src_queue_family_index: vk_queue_family_index,
                                                dst_queue_family_index: vk_queue_family_index,
                                                image: image.handle(),
                                                subresource_range:
                                                    resource::Image::image_subresource_range(
                                                        vk::ImageAspectFlags::COLOR,
                                                    ),
                                                _marker: Default::default(),
                                            },
                                            _marker: Default::default(),
                                        },
                                    );
                                },
                                AssetFinished::Buffer { buffer, .. } => unsafe {
                                    device.get_handle().cmd_copy_buffer(
                                        vk_command_buffer.handle(),
                                        staging_buffer.handle(),
                                        buffer.handle(),
                                        &[vk::BufferCopy {
                                            src_offset: 0,
                                            dst_offset: 0,
                                            size: data_size as vk::DeviceSize,
                                        }],
                                    )
                                },
                            }
                            let raw_command_buffer = vk_command_buffer.handle();
                            let vk_command_buffer = vk_command_buffer.end()?;
                            _command_buffer = Some(vk_command_buffer
                                .submit(
                                    vk_guard.handle(),
                                    &[dagal::command::CommandBufferExecutable::submit_info_sync(
                                        &[dagal::command::CommandBufferExecutable::submit_info(
                                            raw_command_buffer,
                                        )],
                                        &[],
                                        &[],
                                    )],
                                    vk_fence.handle(),
                                )
                                .unwrap());
                            drop(vk_guard);
                        }
                        vk_fence.await?;
                        drop(staging_buffer);
                        gpu_memory_pool.add_permits(data_size);
                        Ok(resource)
                    })
                })
                .collect();
            let mut futures = stream::FuturesUnordered::new();
            for task in tasks {
                futures.push(task);
            }
            let mut images: Vec<Option<(resource::Image<A>, resource::ImageView)>> =
                (0..gltf.images().len()).map(|_| None).collect();
            let mut accessors: HashMap<AccessorIndex, resource::Buffer<A>> = HashMap::new();
            loop {
                tokio::select! {
                    Some(asset) = futures.next() => {
                        let asset = asset??;
                        match asset {
                            AssetFinished::Image{ index,image,image_view  } => {
                                println!("Loaded image {index}");
                                images[index] = Some((image, image_view));
                            },
                            AssetFinished::Buffer{index,buffer } => {
                                println!("Loaded accessor {}", index.accessor_index);
                                accessors.insert(index, buffer);
                            }
                        }
                    },
                    else => {
                        break
                    }
                }
            }
            let images: Vec<(handle::ImageHandle<A>, handle::ImageViewHandle<A>)> = images
                .into_iter()
                .enumerate()
                .map(|(_, mut image)| {
                    let image = image.take().unwrap();
                    let image_handle = gpu_rt.new_image(
                        ResourceInput::Resource(image.0),
                        ResourceInput::Resource(image.1),
                        vk::ImageLayout::GENERAL,
                    )?;
                    let image_handle = (
                        handle::ImageHandle::new(image_handle.0, gpu_rt.clone()),
                        handle::ImageViewHandle::new(image_handle.1, gpu_rt.clone()),
                    );

                    Ok(image_handle)
                })
                .collect::<Result<Vec<(handle::ImageHandle<A>, handle::ImageViewHandle<A>)>>>()?;
            let accessors: HashMap<AccessorIndex, Handle<resource::Buffer<A>>> = accessors
                .into_iter()
                .map(|(key, buffer)| {
                    gpu_rt
                        .new_buffer(ResourceInput::Resource(buffer))
                        .map(|new_buffer| (key, new_buffer))
                })
                .collect::<Result<HashMap<AccessorIndex, Handle<resource::Buffer<A>>>>>()?;

            (images, accessors)
        };
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
                                    assert_eq!(
                                        indices.dimensions(),
                                        gltf::accessor::Dimensions::Scalar
                                    );
                                    let positions =
                                        primitive.get(&gltf::Semantic::Positions).unwrap();
                                    assert_eq!(
                                        positions.dimensions(),
                                        gltf::accessor::Dimensions::Vec3
                                    );
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
                                                    .remove(&AccessorIndex {
                                                        mesh_index: mesh.index(),
                                                        primitive_index: primitive.index(),
                                                        accessor_index: indices.index(),
                                                    })
                                                    .unwrap(),
                                                positions: accessors
                                                    .remove(&AccessorIndex {
                                                        mesh_index: mesh.index(),
                                                        primitive_index: primitive.index(),
                                                        accessor_index: positions.index(),
                                                    })
                                                    .unwrap()
                                                    .clone(),
                                                normals: normal.map(|accessor| {
                                                    accessors
                                                        .remove(&AccessorIndex {
                                                            mesh_index: mesh.index(),
                                                            primitive_index: primitive.index(),
                                                            accessor_index: accessor.index(),
                                                        })
                                                        .unwrap()
                                                }),
                                                uv: uv.map(|accessor| {
                                                    accessors
                                                        .remove(&AccessorIndex {
                                                            mesh_index: mesh.index(),
                                                            primitive_index: primitive.index(),
                                                            accessor_index: accessor.index(),
                                                        })
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
