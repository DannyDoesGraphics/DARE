use std::fmt::Debug;
use std::fs;
use std::io::Read;

use anyhow::{Context, Result};
use ash::vk;

use crate::traits::Destructible;

pub trait Pipeline: Destructible + Debug {
    /// Get a copy to the underlying handle of the struct
    fn handle(&self) -> vk::Pipeline;

    fn get_device(&self) -> &crate::device::LogicalDevice;
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

    fn replace_shader_from_spirv_file(
        self,
        device: crate::device::LogicalDevice,
        path: std::path::PathBuf,
        stage: vk::ShaderStageFlags,
    ) -> Result<Self, (Self, anyhow::Error)> {
        let mut file = fs::File::open(path).context("Failed to open file").unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .context("Failed to read file")
            .unwrap();
        if buffer.len() % 4 != 0 {
            return Err((
                self,
                anyhow::anyhow!("SPIR-V file size is not a multiple of 4"),
            ));
        }
        let u32_content: Vec<u32> = buffer
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        self.replace_shader_from_spirv(device, &u32_content, stage)
    }

    /// Loads and replaces a shader based on source code (**not .spv**) from a file
    fn replace_shader_from_source_file<T: crate::shader::ShaderCompiler>(
        self,
        device: crate::device::LogicalDevice,
        compiler: &T,
        path: std::path::PathBuf,
        stage: vk::ShaderStageFlags,
    ) -> Result<Self, (Self, anyhow::Error)> {
        let content = fs::read_to_string(path.clone());
        if content.is_err() {
            let err = content.unwrap_err();
            return Err((self, anyhow::Error::from(err)));
        }
        let content = content.unwrap();
        let content = compiler.compile(
            content.as_str(),
            crate::shader::ShaderKind::from(stage),
            path.to_str().unwrap(),
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
