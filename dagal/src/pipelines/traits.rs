use crate::traits::Destructible;
use anyhow::Result;
use ash::vk;
use std::fmt::Debug;
use std::fs;

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

    /// Loads and replaces a shader based on SPIR-V content
    fn replace_shader_from_spirv(
        self,
        device: crate::device::LogicalDevice,
        content: &[u32],
        stage: vk::ShaderStageFlags,
    ) -> Result<Self, (Self, anyhow::Error)> {
        let shader = crate::shader::Shader::from_slice(device, content);
        if shader.is_err() {
            let err = shader.unwrap_err();
            return Err((self, err));
        }
        let shader = shader.unwrap();
        Ok(self.replace_shader(shader, stage))
    }

    /// Loads and replaces a shader based on source code (**not .spv**) from a file
    fn replace_shader_from_source_file<T: crate::shader::ShaderCompiler>(
        self,
        device: crate::device::LogicalDevice,
        compiler: &T,
        path: std::path::PathBuf,
        stage: vk::ShaderStageFlags,
    ) -> Result<Self, (Self, anyhow::Error)> {
        /// do not care, force compilation of files
        let content = fs::read_to_string(path);
        if content.is_err() {
            let err = content.unwrap_err();
            return Err((self, anyhow::Error::from(err)));
        }
        let content = content.unwrap();
        let content = compiler.compile(
            content.as_str(),
            crate::shader::ShaderKind::from(stage),
            "asdasd",
        );
        if content.is_err() {
            let err = content.unwrap_err();
            return Err((self, err));
        };
        let content = content.unwrap();
        self.replace_shader_from_spirv(device, content.as_slice(), stage)
    }

    /// Build the pipeline
    fn build(self, device: crate::device::LogicalDevice) -> Result<Self::BuildTo>;
}
