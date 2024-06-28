use std::{path, ptr};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::Arc;

use anyhow::Result;
use bytemuck::{cast_slice, Pod};
use gltf::Gltf;
use gltf::texture::{MagFilter, WrappingMode};
use image::{ColorType, EncodableLayout, GenericImageView};
use rayon::prelude::*;
use tracing::warn;

use dagal::allocators::{Allocator, ArcAllocator, MemoryLocation};
use dagal::ash::vk;
use dagal::command::command_buffer::CmdBuffer;
use dagal::descriptor::bindless::bindless::ResourceInput;
use dagal::descriptor::GPUResourceTable;
use dagal::pipelines::GraphicsPipeline;
use dagal::resource;
use dagal::resource::traits::{Nameable, Resource};
use dagal::util::free_list_allocator::Handle;
use dagal::util::ImmediateSubmit;

use crate::{assets, render, util};
use crate::util::handle;

/// Responsible for loading gltf assets

/// Struct which loads GLTF assets
#[derive(Debug)]
pub struct GltfLoader<'a> {
    immediate: &'a mut ImmediateSubmit,
}

/// Represents a flatten node
struct FlattenNode<'a> {
    handle: gltf::Node<'a>,
    transform: glam::Mat4,
}

/// Same as [`gltf::Semantic`], but contains an extra enum for index usage
#[derive(Clone, Hash, PartialEq, Eq)]
enum AccessorUsageFlags {
    Index,
    Semantic(gltf::Semantic),
}

/// Tracks accessors and their usages
struct AccessorUsages<'a> {
    handle: gltf::Accessor<'a>,
    usage: HashSet<AccessorUsageFlags>,
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

    async fn load_semantic<'b>(meshes: &[gltf::Mesh<'b>], usage: AccessorUsageFlags) -> Vec<f32> {
        for mesh in meshes {
            for primitive in mesh.primitives() {
                let accessor: gltf::Accessor = match &usage {
                    AccessorUsageFlags::Index => primitive.indices().unwrap(),
                    AccessorUsageFlags::Semantic(semantic) => primitive.get(semantic).unwrap(),
                };
            }
        }
        vec![]
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
    pub fn load_assets<A: Allocator>(
        &mut self,
        allocator: &mut ArcAllocator<A>,
        mut gpu_rt: GPUResourceTable<A>,
        path: path::PathBuf,
        pipeline: Arc<render::pipeline::Pipeline<GraphicsPipeline>>,
    ) -> Result<(Vec<Arc<render::Material<A>>>, Vec<assets::mesh::Mesh<A>>)> {
        let gltf = Gltf::open(path.clone())?;
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
        let images: Result<Vec<image::DynamicImage>> = gltf
            .images()
            .collect::<Vec<gltf::image::Image>>()
            .into_par_iter()
            .map(|image| Self::load_image(gltf.blob.as_deref(), image, parent_path.clone()))
            .collect();
        let images: Vec<image::DynamicImage> = images.unwrap();
        let images: Result<Vec<(handle::ImageHandle<A>, handle::ImageViewHandle<A>)>> = gltf
            .images()
            .zip(images)
            .map(|(gltf_image, image)| {
                let image = image.to_rgba8();
                let format = vk::Format::R8G8B8A8_SRGB;

                let mip_levels = image.height().ilog2().max(1);
                let device = gpu_rt.get_device().clone();
                let (image_handle, image_view_handle) = gpu_rt.new_image(
                    ResourceInput::ResourceCI(resource::ImageCreateInfo::NewAllocated {
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
                    }),
                    ResourceInput::ResourceCI(resource::ImageViewCreateInfo::FromCreateInfo {
                        device: gpu_rt.get_device().clone(),
                        create_info: vk::ImageViewCreateInfo {
                            s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                            p_next: ptr::null(),
                            flags: vk::ImageViewCreateFlags::empty(),
                            image: vk::Image::null(),
                            view_type: vk::ImageViewType::TYPE_2D,
                            format,
                            components: vk::ComponentMapping::default(),
                            subresource_range: resource::image::Image::image_subresource_range(
                                vk::ImageAspectFlags::COLOR,
                            ),
                            _marker: Default::default(),
                        },
                    }),
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                )?;
                gpu_rt.with_image(&image_handle, |vk_image| {
                    let mut staging_buffer =
                        resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                            device: gpu_rt.get_device().clone(),
                            allocator,
                            size: std::mem::size_of_val(image.as_bytes()) as vk::DeviceAddress,
                            memory_type: MemoryLocation::CpuToGpu,
                            usage_flags: vk::BufferUsageFlags::TRANSFER_SRC,
                        })?;
                    staging_buffer.write(0, image.as_bytes())?;
                    self.immediate.submit(|ctx| {
                        vk_image.transition(
                            ctx.cmd,
                            ctx.queue,
                            vk::ImageLayout::UNDEFINED,
                            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        );
                        let copy_info = vk::BufferImageCopy {
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
                            image_extent: vk_image.extent(),
                        };
                        unsafe {
                            ctx.device.get_handle().cmd_copy_buffer_to_image(
                                ctx.cmd.handle(),
                                staging_buffer.handle(),
                                vk_image.handle(),
                                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                &[copy_info],
                            );
                        }
                        vk_image.transition(
                            ctx.cmd,
                            ctx.queue,
                            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        );
                    });
                    Ok::<(), anyhow::Error>(())
                })??;
                Ok((
                    handle::ImageHandle::new(image_handle, gpu_rt.clone()),
                    handle::ImageViewHandle::new(image_view_handle, gpu_rt.clone()),
                ))
            })
            .collect();
        println!("Loaded images");
        let images: Vec<(handle::ImageHandle<A>, handle::ImageViewHandle<A>)> = images?;
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
        let meshes: Vec<assets::mesh::Mesh<A>> = nodes.into_iter().filter_map(|node| {
            if let Some(mesh) = node.handle.mesh() {
                let primitives: Vec<Arc<render::Surface<A>>> = mesh_surfaces.entry(mesh.index()).or_insert_with(|| {
                    mesh.primitives().flat_map(|primitive| {
                        if let (Some(index_accessor), Some(vertex_accessor)) = (primitive.indices(), primitive.get(&gltf::Semantic::Positions)) {
                            let indices: Vec<u8> = Self::load_accessor(gltf.blob.as_deref(), index_accessor.clone(), parent_path.clone());
                            let vertices: Vec<u8> = Self::load_accessor(gltf.blob.as_deref(), vertex_accessor.clone(), parent_path.clone());
                            let uvs: Option<Vec<u8>> = {
                                if let Some(uv_accessor) = primitive.get(&gltf::Semantic::TexCoords(0)) {
                                    let uvs = Self::load_accessor(gltf.blob.as_deref(), uv_accessor.clone(), parent_path.clone());
                                    let uvs = Self::cast_bytes(uv_accessor.data_type(), gltf::accessor::DataType::F32, uvs);
                                    Some(uvs)
                                } else {
                                    None
                                }
                            };
                            let indices = Self::cast_bytes(index_accessor.data_type(), gltf::accessor::DataType::U32, indices);
                            let vertices = Self::cast_bytes(vertex_accessor.data_type(), gltf::accessor::DataType::F32, vertices);
                            assert_eq!(vertices.len() % 12, 0);
                            let mut surface = render::Surface::from_primitives(render::SurfaceBuilder {
                                gpu_rt: gpu_rt.clone(),
                                material: materials[primitive.material().index().map(|id| id + 1).unwrap_or(0)].clone(),
                                allocator,
                                immediate: self.immediate,
                                indices,
                                vertices,
                                normals: None,
                                uv: uvs,
                                total_indices: index_accessor.count() as u32,
                                first_index: 0,
                                name: mesh.name().unwrap_or("gltf_primitive"),
                            })
                                .map_err(|e| {
                                    warn!("Unable to primitive surface id: {} on mesh: {:?} (id: {})", primitive.index(), mesh.name(), mesh.index());
                                    e
                                }).unwrap();
                            surface.upload(self.immediate, allocator, glam::Mat4::IDENTITY).unwrap();
                            Some(Arc::new(surface))
                        } else {
                            warn!("Mesh primitive in {:?} (id: {}) does not have an index buffer. Skipping.", mesh.name(), mesh.index());
                            None
                        }
                    }).collect::<Vec<Arc<render::Surface<A>>>>()
                }).clone();
                Some(assets::mesh::Mesh::new(
                    mesh.name().map(|n| n.to_string()),
                    node.transform.w_axis.truncate(),
                    glam::Vec3::new(
                        node.transform.x_axis.length(),
                        node.transform.y_axis.length(),
                        node.transform.z_axis.length()
                    ),
                    glam::Quat::from_mat4(
                        &node.transform
                    ),
                    primitives
                ))
            } else {
                None
            }
        }).collect();
        println!("Loaded meshes");
        Ok((materials, meshes))
    }
}
