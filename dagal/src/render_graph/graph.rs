use crate::allocators::Allocator;
use crate::render_graph::resource::storage::PhysicalResourceStorage;
use crate::virtual_resource::VirtualResource;
use std::collections::HashMap;
use std::marker::PhantomData;

#[derive(Debug)]
pub enum NodeType<'a> {
    ComputeNode(super::pass::ComputePassNode<'a>),
    RasterNode(),
    RayTracingNode(),
    TransferNode(),
    _phantom { _marker: PhantomData<&'a ()> },
}

#[derive(Debug)]
pub struct RenderGraph<'a, A: Allocator = crate::DefaultAllocator> {
    pub(crate) virtual_resource_generation: HashMap<VirtualResource, u32>,
    pub(crate) physical_resource_storage: PhysicalResourceStorage<'a>,
    /// Counter for virtual resources' id
    virtual_resource_counter: u64,
    /// Graph
    pub(crate) graph: petgraph::graph::DiGraph<NodeType<'a>, VirtualResource>,
    _phantom_data: PhantomData<A>,
}

impl<'a, A: Allocator> RenderGraph<'a, A> {
    pub fn new() -> Self {
        Self {
            virtual_resource_generation: HashMap::new(),
            physical_resource_storage: PhysicalResourceStorage::new(),
            virtual_resource_counter: 0,
            graph: Default::default(),
            _phantom_data: Default::default(),
        }
    }

    fn increment_resource_uid(&mut self) -> u64 {
        let uid = self.virtual_resource_counter;
        self.virtual_resource_counter += 1;
        uid
    }

    fn create_virtual_resource<T: 'static>(&mut self) -> VirtualResource {
        VirtualResource {
            uid: self.increment_resource_uid(),
            generation: 0,
            type_id: std::any::TypeId::of::<T>(),
        }
    }

    /// Import an already existing buffer into the render graph
    pub fn import_buffer(&mut self, buffer: &'a crate::resource::Buffer<A>) -> VirtualResource {
        let vr = self.create_virtual_resource::<crate::resource::Buffer<A>>();
        self.physical_resource_storage
            .resources
            .insert(vr, Box::new(buffer));

        vr
    }

    /// Import an already existing texture into the render graph
    pub fn import_texture(&mut self, texture: &'a crate::resource::Image<A>) -> VirtualResource {
        let vr = self.create_virtual_resource::<crate::resource::Image<A>>();
        self.physical_resource_storage
            .resources
            .insert(vr, Box::new(texture));
        vr
    }

    /// Create a virtual buffer that can be instanced later
    pub fn create_buffer(
        &mut self,
        virtual_resource: super::resource::BufferVirtualResource,
    ) -> VirtualResource {
        let vr = self.create_virtual_resource::<crate::resource::Buffer<A>>();
        self.physical_resource_storage
            .virtual_resource_metadata
            .insert(vr, Box::new(virtual_resource));
        vr
    }

    /// Create a virtual texture that can be instanced later
    pub fn create_texture(
        &mut self,
        virtual_resource: super::resource::TextureVirtualResource,
    ) -> VirtualResource {
        let vr = self.create_virtual_resource::<crate::resource::Image<A>>();
        self.physical_resource_storage
            .virtual_resource_metadata
            .insert(vr, Box::new(virtual_resource));
        vr
    }

    /// Returns the old generation of the virtual resource and increments it
    pub(crate) fn increment_virtual_resource_generation(
        &mut self,
        virtual_resource: &VirtualResource,
    ) -> u32 {
        let generation = self
            .virtual_resource_generation
            .entry(virtual_resource.clone())
            .or_insert(0);
        let generation_old = *generation;
        *generation += 1;
        generation_old
    }

    /// Add a pass to the render graph
    pub(crate) fn add_pass(&mut self, pass: NodeType) {}
}
