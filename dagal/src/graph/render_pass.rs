/// Makes a new render pass
pub trait Pass {
    fn get_resource_outputs(&self) -> &[super::virtual_resource::VirtualResource];
}
