use std::collections::HashMap;
use std::ops::Deref;
use crate::allocators::Allocator;
use crate::graph::pass::Pass;
use crate::graph::virtual_resource::{ResourceHandle, ResourceHandleUntyped};
use crate::pipelines::Pipeline;
use crate::resource::traits::Resource;
use anyhow::Result;
use petgraph::graph::Edge;
use petgraph::prelude::NodeIndex;
use petgraph::visit::{IntoNodeReferences, NodeRef};

/// Contains the actual graph itself

#[derive(Debug)]
pub struct Graph {
    pub(crate) graph: petgraph::graph::DiGraph<Box<Pass<dyn Pipeline>>, ResourceHandleUntyped>,
    /// Maps resource handles back to their nodes
    pub(crate) edge_map: HashMap<ResourceHandleUntyped, NodeIndex>,
    pub(crate) next_handle_id: u32,
}
impl Default for Graph {
    fn default() -> Self {
        Self {
            graph: Default::default(),
            edge_map: Default::default(),
            next_handle_id: 0,
        }
    }
}
impl Graph {
    /// Inserts a pass in
    pub fn insert_pass(&mut self, pass: Pass<dyn Pipeline>) {
        let pass_in = pass.resource_in.clone();
        let node = self.graph.add_node(Box::new(pass));
        for virtual_resource in pass_in {
            self.edge_map.insert(virtual_resource.deref().clone(), node);
        }
    }

    /// Create a new resource
    pub fn create_resource_handle<T: Resource + 'static>(&mut self) -> ResourceHandleUntyped {
        let resource_handle: ResourceHandle<T> = ResourceHandle::<T>::new(
            self.next_handle_id,
            0
        );
        self.next_handle_id += 1;
        resource_handle.into()
    }

    /// Import resources
    pub fn import_buffers<A: Allocator>(&mut self, resources: &[crate::resource::Buffer<A>]) -> Vec<ResourceHandle<
        crate::resource::Buffer<A>
    >> {
        resources.iter().map(|resource| {
            let handle = ResourceHandle::new(self.next_handle_id, 0);
            self.next_handle_id += 1;
            handle
        }).collect::<Vec<ResourceHandle<
            crate::resource::Buffer<A>
        >>>()
    }

    /// Import images
    pub fn import_images<A: Allocator>(&mut self, resources: &[crate::resource::Image<A>]) -> Vec<ResourceHandle<
        crate::resource::Image<A>
    >> {
        resources.iter().map(|resource| {
            let handle = ResourceHandle::new(self.next_handle_id, 0);
            self.next_handle_id += 1;
            handle
        }).collect::<Vec<ResourceHandle<
            crate::resource::Image<A>
        >>>()
    }

    /// Create render pass managed buffer
    pub fn new_buffers<A: Allocator>(&mut self, amount: u32) -> Vec<ResourceHandle<
        crate::resource::Buffer<A>
    >> {
        (0..amount).map(|_| {
            let handle = ResourceHandle::new(self.next_handle_id, 0);
            self.next_handle_id += 1;
            handle
        }).collect()
    }

    /// Create render pass managed images
    pub fn new_images<A: Allocator>(&mut self, amount: u32) -> Vec<ResourceHandle<
        crate::resource::Image<A>
    >> {
        (0..amount).map(|_| {
            let handle = ResourceHandle::new(self.next_handle_id, 0);
            self.next_handle_id += 1;
            handle
        }).collect()
    }
}

/// Execution of the graph
impl Graph {
    /// Build the graph
    pub fn build(mut self) -> Self {

        for (node_index, pass) in self.graph.node_references() {
            self.graph.add_edge(
                node_index,
            );
        }

        self
    }
    /// Execute the graph
    pub fn execute(&mut self) -> Result<()> {
        todo!()
    }
}