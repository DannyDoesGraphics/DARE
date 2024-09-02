use std::ptr;

use anyhow::Result;
use ash::vk;
use ash::vk::Handle;
use derivative::Derivative;

use crate::resource::traits::{Nameable, Resource};
use crate::traits::AsRaw;

#[derive(Copy, Clone, Debug)]
pub enum DescriptorInfo {
    Buffer(vk::DescriptorBufferInfo),
    Image(vk::DescriptorImageInfo),
}

impl Default for DescriptorInfo {
    fn default() -> Self {
        Self::Buffer(Default::default())
    }
}

/// https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkDescriptorType.html
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Eq, Ord, Hash)]
pub enum DescriptorType {
    Sampler = 0,
    CombinedImageSampler = 1,
    SampledImage = 2,
    StorageImage = 3,
    UniformTexelBuffer = 4,
    StorageTexelBuffer = 5,
    UniformBuffer = 6,
    StorageBuffer = 7,
    UniformBufferDynamic = 8,
    StorageBufferDynamic = 9,
    InputAttachment = 10,
}

impl Default for DescriptorType {
    fn default() -> Self {
        Self::Sampler
    }
}

impl DescriptorType {
    pub fn to_vk(&self) -> vk::DescriptorType {
        vk::DescriptorType::from_raw(*self as i32)
    }
}

#[derive(Clone, Default, Derivative)]
#[derivative(Debug)]
pub struct DescriptorWriteInfo {
    pub slot: u32,
    pub binding: u32,
    pub ty: DescriptorType,
    #[derivative(Debug = "ignore")]
    pub descriptors: Vec<DescriptorInfo>,
}

impl DescriptorWriteInfo {
    pub fn slot(mut self, slot: u32) -> Self {
        self.slot = slot;
        self
    }

    pub fn binding(mut self, binding: u32) -> Self {
        self.binding = binding;
        self
    }

    pub fn ty(mut self, ty: DescriptorType) -> Self {
        self.ty = ty;
        self
    }

    pub fn descriptors(mut self, descriptors: Vec<DescriptorInfo>) -> Self {
        self.descriptors = descriptors;
        self
    }

    pub fn push_descriptor(mut self, descriptor: DescriptorInfo) -> Self {
        self.descriptors.push(descriptor);
        self
    }

    pub fn push_descriptors(mut self, mut descriptor: Vec<DescriptorInfo>) -> Self {
        self.descriptors.append(&mut descriptor);
        self
    }
}

#[derive(Debug, Clone)]
pub struct DescriptorSet {
    handle: vk::DescriptorSet,
    device: crate::device::LogicalDevice,
}

pub enum DescriptorSetCreateInfo<'a> {
    FromVk {
        handle: vk::DescriptorSet,

        device: crate::device::LogicalDevice,
        name: Option<&'a str>,
    },
    NewSet {
        pool: &'a crate::descriptor::DescriptorPool,
        layout: &'a crate::descriptor::DescriptorSetLayout,

        name: Option<&'a str>,
    },
}

impl DescriptorSet {
    pub fn new(descriptor_set_ci: DescriptorSetCreateInfo) -> Result<Self> {
        match descriptor_set_ci {
            DescriptorSetCreateInfo::FromVk {
                handle,
                device,
                name,
            } => {
                let mut handle = Self { handle, device };
                if let Some(debug_utils) = handle.device.clone().get_debug_utils() {
                    if let Some(name) = name {
                        handle.set_name(debug_utils, name)?;
                    }
                }

                Ok(handle)
            }
            DescriptorSetCreateInfo::NewSet { pool, layout, name } => {
                let alloc_info = vk::DescriptorSetAllocateInfo {
                    s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
                    p_next: ptr::null(),
                    descriptor_pool: unsafe { *pool.as_raw() },
                    descriptor_set_count: 1,
                    p_set_layouts: unsafe { layout.as_raw() },
                    _marker: Default::default(),
                };
                let mut handle = unsafe {
                    pool.get_device()
                        .get_handle()
                        .allocate_descriptor_sets(&alloc_info)?
                };
                Self::new(DescriptorSetCreateInfo::FromVk {
                    handle: handle.pop().unwrap(),
                    device: pool.get_device().clone(),
                    name,
                })
            }
        }
    }

    /// Submit writes to the current descriptor set
    pub fn write(&self, writes: &[DescriptorWriteInfo]) {
        let mut descriptor_writes: Vec<vk::WriteDescriptorSet> = Vec::with_capacity(writes.len());
        let mut descriptor_buffer_infos: Vec<vk::DescriptorBufferInfo> =
            Vec::with_capacity(writes.len());
        let mut descriptor_image_infos: Vec<vk::DescriptorImageInfo> =
            Vec::with_capacity(writes.len());

        for write in writes.iter() {
            let descriptor_write = vk::WriteDescriptorSet {
                s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
                p_next: ptr::null(),
                dst_set: self.handle,
                dst_binding: write.binding,
                dst_array_element: write.slot,
                descriptor_count: 0,
                descriptor_type: write.ty.to_vk(),
                p_image_info: ptr::null(),
                p_buffer_info: ptr::null(),
                p_texel_buffer_view: ptr::null(),
                _marker: Default::default(),
            };
            match write.ty {
                DescriptorType::Sampler
                | DescriptorType::CombinedImageSampler
                | DescriptorType::SampledImage
                | DescriptorType::StorageImage
                | DescriptorType::InputAttachment => {
                    let mut descriptor_count: u32 = 0;
                    let start: usize = descriptor_image_infos.len();
                    for descriptor in write.descriptors.iter() {
                        if let DescriptorInfo::Image(descriptor) = descriptor {
                            descriptor_count += 1;
                            descriptor_image_infos.push(*descriptor)
                        }
                    }

                    let mut descriptor_write = descriptor_write;
                    descriptor_write.descriptor_count = descriptor_count;
                    descriptor_write.p_image_info = descriptor_image_infos[start..].as_ptr();
                    descriptor_writes.push(descriptor_write);
                }
                DescriptorType::UniformBuffer
                | DescriptorType::StorageBuffer
                | DescriptorType::UniformBufferDynamic
                | DescriptorType::StorageBufferDynamic => {
                    let mut descriptor_count: u32 = 0;
                    let start: usize = descriptor_buffer_infos.len();
                    for descriptor in write.descriptors.iter() {
                        if let DescriptorInfo::Buffer(descriptor) = descriptor {
                            descriptor_count += 1;
                            descriptor_buffer_infos.push(*descriptor)
                        }
                    }

                    let mut descriptor_write = descriptor_write;
                    descriptor_write.descriptor_count = descriptor_count;
                    descriptor_write.p_buffer_info = descriptor_buffer_infos[start..].as_ptr();
                    descriptor_writes.push(descriptor_write);
                }
                _ => unimplemented!(),
            }
        }

        unsafe {
            self.device
                .get_handle()
                .update_descriptor_sets(descriptor_writes.as_slice(), &[]);
        }
    }

    pub fn handle(&self) -> vk::DescriptorSet {
        self.handle
    }

    pub fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }
}

impl Nameable for DescriptorSet {
    const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::DESCRIPTOR_SET;
    fn set_name(&mut self, debug_utils: &ash::ext::debug_utils::Device, name: &str) -> Result<()> {
        crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
        Ok(())
    }
}
