use ash::vk;

use crate::allocators::Allocator;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Description {
    pub name: String,
    pub extent: super::Extent3D,
    pub format: vk::Format,
    pub usage: vk::ImageUsageFlags,
    pub samples: vk::SampleCountFlags,
    pub transient: bool,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct State {
    pub layout: vk::ImageLayout,
    pub stage: vk::PipelineStageFlags2,
    pub access: vk::AccessFlags2,
    pub queue_family: u32,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum AccessFlag {
    SampledImage,
    StorageImage,
    ColorAttachmentWrite,
    DepthAttachRead,
    DepthAttachWrite,
    TransferSrc,
    TransferDst,
    Present,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ImageSubresourceRange {
    Full {
        aspect_mask: vk::ImageAspectFlags,
    },
    Sub {
        aspect_mask: vk::ImageAspectFlags,
        mip_levels: std::ops::Range<u32>,
        array_layers: std::ops::Range<u32>,
    },
}
impl ImageSubresourceRange {
    pub fn to_vk<A: Allocator>(&self) -> vk::ImageSubresourceRange {
        match self {
            ImageSubresourceRange::Full { aspect_mask } => vk::ImageSubresourceRange {
                aspect_mask: *aspect_mask,
                base_mip_level: 0,
                level_count: vk::REMAINING_MIP_LEVELS,
                base_array_layer: 0,
                layer_count: vk::REMAINING_ARRAY_LAYERS,
            },
            ImageSubresourceRange::Sub {
                aspect_mask,
                mip_levels,
                array_layers,
            } => vk::ImageSubresourceRange {
                aspect_mask: *aspect_mask,
                base_mip_level: mip_levels.start,
                level_count: mip_levels.end.checked_sub(mip_levels.start).expect("Invalid mip level range. End must be greater than start"),
                base_array_layer: array_layers.start,
                layer_count: array_layers.end.checked_sub(array_layers.start).expect("Invalid array layer range. End must be greater than start"),
            },
        }
    }

    /// Creates a full subresource range for color aspect.
    pub fn full_color() -> Self {
        ImageSubresourceRange::Full {
            aspect_mask: vk::ImageAspectFlags::COLOR,
        }
    }
}
