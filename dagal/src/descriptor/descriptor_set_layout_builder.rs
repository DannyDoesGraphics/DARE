use anyhow::Result;
use ash::vk;
use std::ffi::c_void;
use std::ptr;

#[derive(Clone, Debug, Default)]
pub struct DescriptorSetLayoutBuilder<'a> {
    bindings: Vec<vk::DescriptorSetLayoutBinding<'a>>,
}

impl<'a> DescriptorSetLayoutBuilder<'a> {
    /// Adds a binding to be built
    pub fn add_binding(mut self, binding: u32, ty: vk::DescriptorType) -> Self {
        self.bindings.push(vk::DescriptorSetLayoutBinding {
            binding,
            descriptor_type: ty,
            descriptor_count: 1,
            stage_flags: Default::default(),
            p_immutable_samplers: ptr::null(),
            _marker: Default::default(),
        });
        self
    }

    /// Clear of all bindings
    pub fn clear(&mut self) {
        self.bindings.clear();
    }

    /// Builds the descriptor layout
    pub fn build(
        mut self,
        device: crate::device::LogicalDevice,
        shader_stages: vk::ShaderStageFlags,
        p_next: *const c_void,
        create_flags: vk::DescriptorSetLayoutCreateFlags,
    ) -> Result<crate::descriptor::DescriptorSetLayout> {
        for binding in self.bindings.iter_mut() {
            binding.stage_flags |= shader_stages;
        }
        let descriptor_set_layout_ci = vk::DescriptorSetLayoutCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
            p_next,
            flags: create_flags,
            binding_count: self.bindings.len() as u32,
            p_bindings: self.bindings.as_ptr(),
            _marker: Default::default(),
        };
        let handle = unsafe {
            device
                .get_handle()
                .create_descriptor_set_layout(&descriptor_set_layout_ci, None)?
        };
        Ok(crate::descriptor::DescriptorSetLayout::from_raw(
            handle, device,
        ))
    }
}
