use std::ffi::c_char;
use std::ptr;

use anyhow::Result;
use ash::vk;

use crate::pipelines::traits::PipelineBuilder;
use crate::traits::Destructible;

#[derive(Debug, Clone)]
pub struct ComputePipeline {
    device: crate::device::LogicalDevice,
    handle: vk::Pipeline,
    layout: vk::PipelineLayout,
}

impl Destructible for ComputePipeline {
    fn destroy(&mut self) {
        unsafe {
            self.device.get_handle().destroy_pipeline(self.handle, None);
        }
    }
}

impl super::Pipeline for ComputePipeline {
    fn handle(&mut self) -> vk::Pipeline {
        self.handle
    }
}

/// Builds the compute pipeline
#[derive(Default, Debug)]
pub struct ComputePipelineBuilder<'a> {
    handle: vk::ComputePipelineCreateInfo<'a>,
    compute_shader: Option<crate::shader::Shader>,
    layout: Option<crate::pipelines::PipelineLayout>,
}

impl<'a> PipelineBuilder for ComputePipelineBuilder<'a> {
    type BuildTo = ComputePipeline;

    /// Destroy and existing layout and replace it
    fn replace_layout(mut self, layout: crate::pipelines::PipelineLayout) -> Self {
        if let Some(mut layout) = self.layout.take() {
            layout.destroy();
        }
        self.layout = Some(layout);
        self
    }

    /// Destroy any existing shader and replace it
    fn replace_shader(mut self, compute_shader: crate::shader::Shader, stages: vk::ShaderStageFlags) -> Self {
        if stages & vk::ShaderStageFlags::COMPUTE == vk::ShaderStageFlags::COMPUTE {
            if let Some(mut shader) = self.compute_shader.take() {
                shader.destroy();
            }
            self.compute_shader = Some(compute_shader);
            self
        } else {
            panic!("Compute shaders only accept VkPipelineStagesFlags::COMPUTE");
        }
    }

    /// Builds the compute pipeline
    fn build(mut self, device: crate::device::LogicalDevice) -> Result<ComputePipeline> {
        assert!(self.compute_shader.is_some());
        assert!(self.layout.is_some());
        self.handle.s_type = vk::StructureType::COMPUTE_PIPELINE_CREATE_INFO;
        self.handle.p_next = ptr::null();
        self.handle.stage = vk::PipelineShaderStageCreateInfo {
            s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::PipelineShaderStageCreateFlags::empty(),
            stage: vk::ShaderStageFlags::COMPUTE,
            module: self.compute_shader.as_ref().unwrap().handle(),
            p_name: "main".as_ptr() as *const c_char,
            p_specialization_info: ptr::null(),
            _marker: Default::default(),
        };
        self.handle.layout = self.layout.unwrap().handle();


        let pipeline = unsafe {
            device.get_handle().create_compute_pipelines(
                vk::PipelineCache::null(),
                &[self.handle],
                None
            ).map_err(|e| anyhow::Error::from(e.1))?.pop().unwrap()
        };
        Ok(ComputePipeline {
            device,
            handle: pipeline,
            layout: Default::default(),
        })
    }
}