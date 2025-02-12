use std::collections::HashMap;
use std::ops::Deref;
use crate::allocators::Allocator;
use crate::graph::pass::Pass;
use crate::graph::virtual_resource::{ResourceHandle, ResourceHandleUntyped, VirtualResourceEdge};
use crate::pipelines::Pipeline;
use crate::resource::traits::Resource;
use anyhow::Result;
use petgraph::algo;
use petgraph::algo::Cycle;
use petgraph::prelude::NodeIndex;
use petgraph::visit::{IntoNodeReferences, NodeRef};

/// Contains the actual graph itself

#[derive(Debug)]
pub struct Graph {
    pub(crate) graph: petgraph::graph::DiGraph<Box<Pass<dyn Pipeline>>, ResourceHandleUntyped>,
    /// Maps resource handles back to their nodes
    pub(crate) next_handle_id: u32,
}
impl Default for Graph {
    fn default() -> Self {
        Self {
            graph: Default::default(),
            next_handle_id: 0,
        }
    }
}
impl Graph {
    /// Inserts a pass in
    pub fn insert_pass<T: Pipeline + 'static>(&mut self, pass: Box<Pass<T>>) {

        // SAFETY: so long as `Pass<T>`, does not actually contain the underlying pass
        // and only uses it for type safety, we *should* be fine transmuting this
        let pass: Box<Pass<dyn Pipeline>> = unsafe {
            std::mem::transmute(pass)
        };
        let pass_in = pass.resource_in.clone();
        let node = self.graph.add_node(pass);
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
        // connect the graph together
        let mut resource_mappings: HashMap<ResourceHandleUntyped, Vec<NodeIndex<u32>>> = HashMap::default();
        let mut pass_dependency_mappings: HashMap<NodeIndex<u32>, Vec<ResourceHandleUntyped>> = HashMap::default();
        for (node_index, pass) in self.graph.node_references() {
            for edge in pass.resource_out.iter() {
                resource_mappings.entry(edge.clone()).or_insert_with(Vec::new).push(node_index.clone());
            };
            for edge in pass.resource_in.iter() {
                match edge {
                    VirtualResourceEdge::Read(r) => {
                        pass_dependency_mappings.entry(node_index.clone()).or_insert_with(Vec::new).push(r.clone());
                    }
                    VirtualResourceEdge::Write(w) => {
                        pass_dependency_mappings.entry(node_index.clone()).or_insert_with(Vec::new).push(w.clone());
                    }
                    VirtualResourceEdge::ReadWrite(rw) => {
                        pass_dependency_mappings.entry(node_index.clone()).or_insert_with(Vec::new).push(rw.clone());
                    }
                }
            }
        }
        println!("{:?}\n{:?}", resource_mappings, pass_dependency_mappings);
        // link versioned
        for (node_index_dependent, pass_dependencies) in pass_dependency_mappings {
            for dependency in pass_dependencies {
                match resource_mappings.get(&dependency) {
                    None => {
                        if dependency.generation != 0 {
                            panic!("Tried finding a non-root resource dependency, does not exist.");
                        }
                    }
                    Some(nodes) => for node in nodes {
                        self.graph.add_edge(node.clone(), node_index_dependent, dependency.clone());
                    }
                }
            }
        }
        self
    }
    /// Execute the graph
    pub fn execute(&mut self) -> Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use crate::allocators::GPUAllocatorImpl;
    use crate::pipelines::GraphicsPipeline;
    use crate::resource::Buffer;
    use super::*;

    // test a basic two pass and 1 dependency between
    #[test]
    pub fn two_nodes() {
        let mut graph = Graph::default();
        let mut pass: Pass<GraphicsPipeline> = Pass::default();
        let mut pass_2: Pass<GraphicsPipeline> = Pass::default();
        let buffer: ResourceHandle<Buffer<GPUAllocatorImpl>> = graph.new_buffers(1).pop().unwrap();
        let mut pass = pass.write(buffer.clone().into());
        let buffer = pass.output_typed(buffer).unwrap();
        let pass_2 = pass_2.read(&buffer.into());
        graph.insert_pass(Box::new(pass));
        graph.insert_pass(Box::new(pass_2));
        graph.build();
    }

    /// Test if using the same resources twice on the same pass would induce a panic
    #[test]
    #[should_panic]
    pub fn node_same_resource() {
        let mut graph = Graph::default();
        let mut pass: Pass<GraphicsPipeline> = Pass::default();
        let buffer: ResourceHandle<Buffer<GPUAllocatorImpl>> = graph.new_buffers(1).pop().unwrap();
        let mut pass = pass.write(buffer.clone().into());
        let buffer = pass.output_untyped(buffer.clone().into()).unwrap();
        pass.write(buffer);
    }

    /// Test using two nodes, to check for a cycle
    #[test]
    #[should_panic]
    pub fn two_nodes_cycle() {
        let mut graph = Graph::default();
        let mut pass: Pass<GraphicsPipeline> = Pass::default();
        let mut pass_2: Pass<GraphicsPipeline> = Pass::default();

        // init resources
        let buffer: ResourceHandle<Buffer<GPUAllocatorImpl>> = graph.new_buffers(1).pop().unwrap();
        // indicate a write root dependency
        let mut pass = pass.write(buffer.clone().into());
        let buffer = pass.output_typed(buffer).unwrap();
        // indicate a write dependency
        // b -> pass_1 -> b -> pass_2
        let mut pass_2 = pass_2.write(buffer.clone().into());
        let buffer = pass_2.output_typed(buffer).unwrap();
        // make a loop back
        // b -> pass_1 -> b -> pass_2 -> b -> pass_1
        let pass = pass.write(buffer.into());
        graph.insert_pass(Box::new(pass));
        graph.insert_pass(Box::new(pass_2));
        graph.build();
    }
}