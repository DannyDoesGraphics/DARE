use std::collections::HashSet;
use std::fmt::Debug;

use crate::resource::traits::Resource;

use super::{
    RenderGraph, ResourceAccess, UntypedNoGenerationVirtualResource, UntypedVirtualResource,
    VirtualResource,
};

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct ResourceUse {
    pub(crate) resource: UntypedVirtualResource,
    pub(crate) access: ResourceAccess,
}

#[derive(Debug)]
pub struct PassContext<'a> {
    pub device: crate::device::LogicalDevice,
    pub command_buffer: &'a crate::command::CommandBufferRecording,
}

/// Configures a pass' name and execution independent of any [`RenderGraph`]
#[derive(Default)]
pub struct PassConfiguration<'a> {
    name: Option<String>,
    execution: Option<Box<dyn Fn(PassContext) -> anyhow::Result<()> + 'a>>,
}
impl<'a> PassConfiguration<'a> {
    pub fn name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn execution<F: Fn(PassContext) -> anyhow::Result<()> + 'a>(mut self, f: F) -> Self {
        self.execution = Some(Box::new(f));
        self
    }
}

/// Declares a pass' resource reads/writes against a [`RenderGraph`]
pub struct PassBuilder<'graph, 'a> {
    graph: &'graph mut RenderGraph<'a>,
    configuration: PassConfiguration<'a>,
    pass_index: usize,
    /// What the pass has written to; used to defend against double writes
    written: HashSet<UntypedNoGenerationVirtualResource>,
    /// What resource pass depends on
    dependencies: HashSet<usize>,
    /// Input resources to pass
    inputs: Vec<ResourceUse>,
    /// Input resources out of pass
    outputs: Vec<ResourceUse>,
}

impl<'graph, 'a> PassBuilder<'graph, 'a> {
    pub(crate) fn new(
        graph: &'graph mut RenderGraph<'a>,
        configuration: PassConfiguration<'a>,
    ) -> Self {
        let pass_index = graph.pass_count();
        Self {
            graph,
            configuration,
            pass_index,
            written: HashSet::new(),
            dependencies: HashSet::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// Give pass read access to a given resource
    pub fn read<A: Resource + 'static>(
        &mut self,
        handle: VirtualResource<A>,
        access: ResourceAccess,
    ) -> VirtualResource<A> {
        let identity: UntypedNoGenerationVirtualResource = handle.into();
        let dependencies = self.graph.record_read(identity, self.pass_index);
        self.dependencies.extend(dependencies);
        self.inputs.push(ResourceUse {
            resource: handle.into(),
            access,
        });
        handle
    }

    /// Give pass write access to a given resource
    pub fn write<A: Resource + 'static>(
        &mut self,
        handle: VirtualResource<A>,
        access: ResourceAccess,
    ) -> VirtualResource<A> {
        let identity: UntypedNoGenerationVirtualResource = handle.into();
        assert!(
            self.written.insert(identity),
            "a pass cannot write the same resource twice in one pass"
        );
        let dependencies = self.graph.record_write(identity, self.pass_index);
        self.dependencies.extend(dependencies);
        self.outputs.push(ResourceUse {
            resource: handle.into(),
            access,
        });
        handle
    }

    /// Complete pass
    pub fn finish(self) {
        self.graph.push_pass(Pass {
            name: self.configuration.name.expect("pass name not set"),
            dependencies: self.dependencies,
            inputs: self.inputs,
            outputs: self.outputs,
            execution: self
                .configuration
                .execution
                .expect("pass execution not set"),
        });
    }
}

/// A [`RenderGraph`] defines a DAG to help with automatic dependencies
pub struct Pass<'a> {
    pub(crate) name: String,
    /// Pass indices which this pass is dependent on aka incoming edges' vertices
    pub(crate) dependencies: HashSet<usize>,
    /// Input resources
    pub(crate) inputs: Vec<ResourceUse>,
    /// Output resources
    pub(crate) outputs: Vec<ResourceUse>,
    pub(crate) execution: Box<dyn Fn(PassContext) -> anyhow::Result<()> + 'a>,
}
impl<'a> Debug for Pass<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pass")
            .field("name", &self.name)
            .field("dependencies", &self.dependencies)
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .finish()
    }
}

impl<'a> Pass<'a> {
    pub fn name(&self) -> &str {
        &self.name
    }
}
