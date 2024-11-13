use ash::vk;
pub use compute::{ComputePipeline, ComputePipelineBuilder};
pub use graphics::{GraphicsPipeline, GraphicsPipelineBuilder};
pub use pipeline_layout::{PipelineLayout, PipelineLayoutCreateInfo};
pub use pipeline_layout_builder::PipelineLayoutBuilder;
use std::ptr;
pub use traits::*;

pub mod compute;

pub mod traits;

pub mod graphics;
mod pipeline_layout;
pub mod pipeline_layout_builder;

#[derive(PartialEq, Eq, Debug, Hash, Clone, Copy)]
pub struct PipelineInputAssemblyStateCreateInfo {
    pub flags: vk::PipelineInputAssemblyStateCreateFlags,
    pub topology: vk::PrimitiveTopology,
    pub primitive_restart_enable: bool,
}

impl<'a> Into<vk::PipelineInputAssemblyStateCreateInfo<'a>>
    for PipelineInputAssemblyStateCreateInfo
{
    fn into(self) -> vk::PipelineInputAssemblyStateCreateInfo<'a> {
        vk::PipelineInputAssemblyStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
            p_next: ptr::null(),
            flags: self.flags,
            topology: self.topology,
            primitive_restart_enable: self.primitive_restart_enable as u32,
            _marker: Default::default(),
        }
    }
}
