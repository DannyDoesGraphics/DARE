use ash::vk;
pub use compute::{ComputePipeline, ComputePipelineBuilder};
pub use graphics::{GraphicsPipeline, GraphicsPipelineBuilder};
pub use pipeline_layout::{PipelineLayout, PipelineLayoutCreateInfo};
pub use pipeline_layout_builder::PipelineLayoutBuilder;
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

impl From<PipelineInputAssemblyStateCreateInfo> for vk::PipelineInputAssemblyStateCreateInfo<'_> {
    fn from(val: PipelineInputAssemblyStateCreateInfo) -> Self {
        vk::PipelineInputAssemblyStateCreateInfo::default()
            .flags(val.flags)
            .topology(val.topology)
            .primitive_restart_enable(val.primitive_restart_enable)
    }
}
