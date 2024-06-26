use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::Arc;

use anyhow::Result;
use bytemuck::{cast_slice, cast_vec, Pod};
use gltf::Gltf;
use tracing::warn;

use dagal::allocators::{Allocator, ArcAllocator, MemoryLocation};
use dagal::descriptor::GPUResourceTable;
use dagal::pipelines::GraphicsPipeline;
use dagal::resource;
use dagal::util::ImmediateSubmit;

use crate::{assets, render, util};

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
                    nodes.append(&mut Self::flatten_node_tree(transform, node.children().collect()))
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

    fn cast_bytes(from: gltf::accessor::DataType, to: gltf::accessor::DataType, slice: Vec<u8>) -> Vec<u8> {
        use gltf::accessor::DataType;
        match (from, to) {
            (DataType::F32, DataType::F32) => slice,
            (DataType::U16, DataType::U32) => Self::convert_and_cast::<u16, u32>(slice),
            (DataType::U32, DataType::U32) => slice,
            _ => panic!("{:?} -> {:?} not implemented.", from, to),
        }
    }
    /// Loads an accessor's underlying data
    fn load_accessor<'b>(blob: Option<&'b [u8]>, accessor: gltf::Accessor, mut path: std::path::PathBuf) -> Vec<u8> {
        if let Some(view) = accessor.view() {
            let mut buffer: Vec<u8> = Vec::new();
            let blob: &[u8] = match view.buffer().source() {
                gltf::buffer::Source::Bin => {
                    blob.expect("GlTF file expected blob, but no blob provided!")
                }
                gltf::buffer::Source::Uri(uri) => {
                    path.push(std::path::PathBuf::from(uri));
                    std::fs::File::open(&path).unwrap().read_to_end(&mut buffer).unwrap();
                    &buffer
                }
            };
            let total_offset: usize = accessor.offset() + view.offset();
            let element_size: usize = accessor.data_type().size() * accessor.dimensions().multiplicity();
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

    /// Loads meshes from a gltf scene and uploads their buffer info onto the gpu
    pub fn load_assets<A: Allocator>(
        &mut self,
        allocator: &mut ArcAllocator<A>,
        gpu_rt: GPUResourceTable<A>,
        path: std::path::PathBuf,
        pipeline: Arc<render::pipeline::Pipeline<GraphicsPipeline>>,
    ) -> Result<(Vec<Arc<render::Material<A>>>, Vec<assets::mesh::Mesh<A>>)> {
        let gltf = Gltf::open(path.clone())?;
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

        let nodes: Vec<FlattenNode> =
            Self::flatten_node_tree(glam::Mat4::IDENTITY, root_nodes);
        let mut materials: Vec<Arc<render::Material<A>>> = vec![
            {
                let mut material = render::Material::new(
                    allocator,
                    pipeline.clone(),
                    glam::Vec4::from([1.0, 1.0, 1.0, 1.0]),
                    None,
                    None,
                    gpu_rt.get_device().clone(),
                )
                    .unwrap();
                material.upload_material(self.immediate, allocator).unwrap();
                Arc::new(material)
            }
        ];
        materials.append(&mut gltf
            .materials()
            .map(
                |gltf_material| {
                    let mut material = render::Material::new(
                        allocator,
                        pipeline.clone(),
                        glam::Vec4::from(gltf_material.pbr_metallic_roughness().base_color_factor()),
                        None,
                        None,
                        gpu_rt.get_device().clone(),
                    )
                        .unwrap();
                    material.upload_material(self.immediate, allocator).unwrap();
                    Arc::new(material)
                }
            )
            .collect());
        let mut mesh_surfaces: HashMap<usize, Vec<Arc<render::Surface<A>>>> = HashMap::new();
        let path = path.parent().unwrap().to_path_buf();
        let meshes: Vec<assets::mesh::Mesh<A>> = nodes.into_iter().filter_map(|node| {
            if let Some(mesh) = node.handle.mesh() {
                let primitives: Vec<Arc<render::Surface<A>>> = mesh_surfaces.entry(mesh.index()).or_insert_with(|| {
                    mesh.primitives().flat_map(|primitive| {
                        if let (Some(index_accessor), Some(vertex_accessor)) = (primitive.indices(), primitive.get(&gltf::Semantic::Positions)) {
                            let indices = Self::load_accessor(gltf.blob.as_deref(), index_accessor.clone(), path.clone());
                            let vertices = Self::load_accessor(gltf.blob.as_deref(), vertex_accessor.clone(), path.clone());
                            println!("Index: {:?}", index_accessor.data_type());
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
                                uv: None,
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
        Ok((materials, meshes))
    }
}
