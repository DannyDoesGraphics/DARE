/// Makes a new render pass
pub trait Pass {
    /// Get all resources
    fn get_resource_outputs(&self) -> &[super::virtual_resource::VirtualResource];

    /// Add a singular resource
    fn add_resource_input(&mut self, resource: super::virtual_resource::VirtualResource) {
        self.add_resource_inputs(vec![resource]);
    }

    /// Add multiple resources
    fn add_resource_inputs(&mut self, resources: Vec<super::virtual_resource::VirtualResource>);
}
