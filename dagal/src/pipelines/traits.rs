use crate::traits::Destructible;
use anyhow::Result;
use ash::vk;
use std::fmt::Debug;
pub trait Pipeline: Destructible + Debug + Clone {
    /// Get a copy to the underlying handle of the struct
    fn handle(&mut self) -> vk::Pipeline;

    /// Get a copy of the pipeline layout
    fn layout(&self) -> vk::PipelineLayout;
}

pub trait PipelineBuilder: Default + Debug {
    type BuildTo;

    /// Replace all current layouts/layout builders and replace them with current pipeline layout.
    /// Passed in layout will become resource managed by [`Pipeline`] when built.
    ///
    /// **Will destroy any existing pipeline layout attached and clear the pipeline layout builder.**
    fn replace_layout(self, layout: vk::PipelineLayout) -> Self;

    /// Replace a shader at the given shader stage. Passed in shader will become resource
    /// managed by the [`PipelineBuilder`]
    fn replace_shader(self, shader: crate::shader::Shader, stage: vk::ShaderStageFlags) -> Self;

    /// Build the pipeline
    fn build(self, device: crate::device::LogicalDevice) -> Result<Self::BuildTo>;
}
