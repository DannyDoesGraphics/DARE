use crate::graph::virtual_resource::VirtualResource;

/// A pass representing a standard graphics pipeline
#[derive(Debug)]
pub struct GraphicsPass {
    pub resources_in: Vec<VirtualResource>,
    pub resources_out: Vec<VirtualResource>,
}

impl super::render_pass::Pass for GraphicsPass {
    fn get_resource_outputs(&self) -> &[VirtualResource] {
        &self.resources_out
    }

    fn add_resource_inputs(&mut self, resources: Vec<VirtualResource>) {
        self.resources_in.extend(resources);
    }
}
