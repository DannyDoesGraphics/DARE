use std::fmt::Debug;
use ash::vk;
use crate::traits::Destructible;
use anyhow::Result;
pub trait Pipeline: Destructible + Debug + Clone {
    fn handle(&mut self) -> vk::Pipeline;
}

pub trait PipelineBuilder: Default + Debug {
    type BuildTo;

    /// Replace all current layouts/layout builders and replace them with current pipeline layout
    ///
    /// **Will destroy any existing pipeline layout attached and clear the pipeline layout builder.**
    fn replace_layout(self, layout: crate::pipelines::PipelineLayout) -> Self;

    /// Replace a shader at the given shader stage
    fn replace_shader(self,  shader: crate::shader::Shader, stage: vk::ShaderStageFlags) -> Self;

    /// Build the pipeline
    fn build(self, device: crate::device::LogicalDevice) -> Result<Self::BuildTo>;
}