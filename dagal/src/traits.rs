/// Describes Vulkan resources which can be destroyed
pub trait Destructible {
    /// Destroy the resource
    fn destroy(&mut self);
}
