use crate::render_graph::graph::{NodeType, RenderGraph};
use crate::virtual_resource::VirtualResource;
use ash::vk;
use derivative::Derivative;

/// Compute pass builder
#[derive(Derivative)]
#[derivative(Debug)]
pub struct ComputePassBuilder<'a> {
    name: &'a str,
    read: Vec<VirtualResource>,
    write: Vec<VirtualResource>,
    shader_path: Option<&'a std::path::Path>,
    push_constant_range: Option<vk::PushConstantRange>,
    #[derivative(Debug = "ignore")]
    rt_callback: Option<super::RuntimeExecutionCallback<ComputePassRuntime>>,
    rg: &'a mut RenderGraph<'a>,
}

/// Runtime data for the compute pass
#[derive(Derivative)]
#[derivative(Debug)]
pub struct ComputePassRuntime {
    pub(crate) dispatch: Option<(u32, u32, u32)>,
    pub(crate) push_constant: Option<Vec<u8>>,
}

impl ComputePassRuntime {
    /// Dispatch the compute pass with given dimensions
    pub fn dispatch(&mut self, dispatch: (u32, u32, u32)) -> &mut Self {
        self.dispatch = Some(dispatch);
        self
    }

    /// Push constant data to the compute pass
    pub fn push_constant<T: Sized + 'static>(&mut self, data: &T) -> &mut Self {
        let bytes =
            unsafe { std::slice::from_raw_parts(data as *const T as *const u8, size_of::<T>()) };
        self.push_constant = Some(bytes.to_vec());
        self
    }
}

impl<'a> ComputePassBuilder<'a> {
    /// Create a new compute pass
    pub fn new(rg: &'a mut RenderGraph<'a>, name: &'a str) -> Self {
        Self {
            name,
            read: Vec::new(),
            write: Vec::new(),
            shader_path: None,
            push_constant_range: None,
            rt_callback: None,
            rg,
        }
    }

    /// Read from a virtual resource
    fn read(mut self, mut virtual_resource: VirtualResource) -> Self {
        virtual_resource.generation = *self
            .rg
            .virtual_resource_generation
            .entry(virtual_resource)
            .or_insert(0);
        self.read.push(virtual_resource);
        self
    }

    /// Write to a virtual resource
    fn write(mut self, mut virtual_resource: VirtualResource) -> Self {
        virtual_resource.generation = self
            .rg
            .increment_virtual_resource_generation(&virtual_resource)
            + 1;
        self.write.push(virtual_resource);
        self
    }

    /// Push constant range
    pub fn push_constant_range<T: Sized + 'static>(mut self) -> Self {
        self.push_constant_range = Some(vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::COMPUTE,
            offset: 0,
            size: size_of::<T>() as u32,
        });
        self
    }
    /// Add shaders
    pub fn shader(mut self, path: &'a std::path::Path) -> Self {
        self.shader_path = Some(path);
        self
    }
}

impl<'a> super::node::traits::NodeBuilder for ComputePassBuilder<'a> {
    type Node = ComputePassNode<'a>;

    fn submit(self) {
        self.rg
            .graph
            .add_node(NodeType::ComputeNode(ComputePassNode {
                name: self.name,
                read: self.read,
                write: self.write,
                rt_callback: self.rt_callback,
                shader_path: self
                    .shader_path
                    .expect("Shader path must be set for compute pass"),
            }));
    }
}

impl<'a> super::PassBuilder for ComputePassBuilder<'a> {
    type PassRuntimeData = ComputePassRuntime;

    /// Execute the compute pass
    fn execute(mut self, execution: super::RuntimeExecutionCallback<ComputePassRuntime>) -> Self {
        self.rt_callback = Some(execution);
        self
    }
}

/// A node for the compute pass
#[derive(Derivative)]
#[derivative(Debug)]
pub(crate) struct ComputePassNode<'a> {
    pub name: &'a str,
    pub read: Vec<VirtualResource>,
    pub write: Vec<VirtualResource>,
    #[derivative(Debug = "ignore")]
    pub rt_callback: Option<super::RuntimeExecutionCallback<ComputePassRuntime>>,
    pub shader_path: &'a std::path::Path,
}

impl<'a> super::node::traits::Node for ComputePassNode<'a> {
    fn reads(&self) -> &[VirtualResource] {
        &self.read
    }

    fn writes(&self) -> &[VirtualResource] {
        &self.write
    }
}
