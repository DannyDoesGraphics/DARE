use std::ptr;

use anyhow::Result;
use ash::vk;

use crate::resource::traits::Resource;

#[derive(Default, Debug, Clone)]
pub struct PipelineLayoutBuilder {
    push_constant_ranges: Vec<vk::PushConstantRange>,
    descriptor_sets: Vec<vk::DescriptorSetLayout>,
}

impl PipelineLayoutBuilder {
    /// Add a push constant range to be added to the pipeline layout
    pub fn push_push_constant_ranges(
        mut self,
        mut push_constant: Vec<vk::PushConstantRange>,
    ) -> Self {
        self.push_constant_ranges.append(&mut push_constant);
        self
    }

    /// Adds a push constant range using a passed type.
    ///
    /// **It is recommended you only use types which have `#[repr(C)]`**.
    pub fn push_push_constant_struct<T: Sized>(self, stage_flags: vk::ShaderStageFlags) -> Self {
        self.push_push_constant_ranges(vec![vk::PushConstantRange {
            stage_flags,
            offset: 0,
            size: std::mem::size_of::<T>() as u32,
        }])
    }

    /// Add descriptor sets to the pipeline layout
    pub fn push_descriptor_sets(
        mut self,
        mut descriptor_sets: Vec<vk::DescriptorSetLayout>,
    ) -> Self {
        self.descriptor_sets.append(&mut descriptor_sets);
        self
    }

    /// Push a GPU resource table
    pub fn push_bindless_gpu_resource_table(
        mut self,
        bindless: &crate::descriptor::GPUResourceTable,
    ) -> Self {
        self.descriptor_sets
            .push(bindless.get_descriptor_layout().unwrap());
        self
    }

    pub fn build(
        self,
        device: crate::device::LogicalDevice,
        flags: vk::PipelineLayoutCreateFlags,
    ) -> Result<crate::pipelines::PipelineLayout> {
        let pipeline_ci = vk::PipelineLayoutCreateInfo {
            s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
            p_next: ptr::null(),
            flags,
            set_layout_count: self.descriptor_sets.len() as u32,
            p_set_layouts: self.descriptor_sets.as_ptr(),
            push_constant_range_count: self.push_constant_ranges.len() as u32,
            p_push_constant_ranges: self.push_constant_ranges.as_ptr(),
            _marker: Default::default(),
        };
        crate::pipelines::PipelineLayout::new(
            crate::pipelines::PipelineLayoutCreateInfo::CreateInfo {
                create_info: pipeline_ci,
                name: None,
                device,
            }
        )
    }
}
