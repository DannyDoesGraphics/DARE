mod buffer;

use crate::virtual_resource::VirtualResource;
use ash::vk;
use std::fmt::Debug;

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum ResourceAccessType {
    Read(VirtualResource),
    Write(VirtualResource),
}

pub(crate) trait Edge: Debug {}

/// Describes any possible memory transition that can occur in the render graph
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct MemoryEdge {
    dst_stage_mask: vk::PipelineStageFlags2,
    dst_access_flag: vk::AccessFlags2,
    dst_queue_family_index: u32,
}
