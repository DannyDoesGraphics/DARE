use crate::traits::Destructible;
use anyhow::Result;
use ash::vk;
use std::ptr;
use tracing::trace;

/// Allocates descriptor set layouts
#[derive(Debug, Clone)]
pub struct DescriptorPool {
    handle: vk::DescriptorPool,
    device: crate::device::LogicalDevice,
}

/// Indicate the ratio of each descriptor type size
#[derive(Copy, Clone, PartialOrd, PartialEq, Debug)]
pub struct PoolSizeRatio {
    pub descriptor_type: vk::DescriptorType,
    pub ratio: f32,
}

#[derive(Copy, Clone, Default, Debug)]
pub struct PoolSize {
    handle: vk::DescriptorPoolSize,
}
impl PoolSize {
    pub fn descriptor_count(mut self, count: u32) -> Self {
        self.handle.descriptor_count = count;
        self
    }

    pub fn descriptor_type(mut self, ty: vk::DescriptorType) -> Self {
        self.handle.ty = ty;
        self
    }
}

impl DescriptorPool {
    pub fn new_with_pool_sizes(device: crate::device::LogicalDevice, flags: vk::DescriptorPoolCreateFlags, max_sets: u32, pool_sizes: &[PoolSize]) -> Result<Self> {
        let raw_pool_sizes: Vec<vk::DescriptorPoolSize> = pool_sizes.iter().map(|pool_size| {
            pool_size.handle
        }).collect();


        let pool_ci = vk::DescriptorPoolCreateInfo {
            s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
            p_next: ptr::null(),
            flags,
            max_sets,
            pool_size_count: raw_pool_sizes.len() as u32,
            p_pool_sizes: raw_pool_sizes.as_ptr(),
            _marker: Default::default(),
        };

        let handle = unsafe { device.get_handle().create_descriptor_pool(&pool_ci, None)? };

        #[cfg(feature = "log-lifetimes")]
        trace!("Creating VkDescriptorPool {:p}", handle);

        Ok(Self { handle, device })
    }

    pub fn new(
        device: crate::device::LogicalDevice,
        max_sets: u32,
        pool_ratios: &[PoolSizeRatio],
    ) -> Result<Self> {
        let pool_sizes: Vec<PoolSize> = pool_ratios
            .iter()
            .map(|pool_ratio| PoolSize::default().descriptor_type(pool_ratio.descriptor_type).descriptor_count((pool_ratio.ratio * max_sets as f32).ceil() as u32))
            .collect();
        Self::new_with_pool_sizes(device, vk::DescriptorPoolCreateFlags::empty(), max_sets, pool_sizes.as_slice())
    }

    /// Resets a descriptor pool and clears it entirely
    pub fn reset(&mut self, flags: vk::DescriptorPoolResetFlags) -> Result<()> {
        unsafe {
            self.device
                .get_handle()
                .reset_descriptor_pool(self.handle, flags)?
        };
        Ok(())
    }

    pub fn allocate(&self, layout: vk::DescriptorSetLayout) -> Result<vk::DescriptorSet> {
        let alloc_info = vk::DescriptorSetAllocateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
            p_next: ptr::null(),
            descriptor_pool: self.handle,
            descriptor_set_count: 1,
            p_set_layouts: &layout,
            _marker: Default::default(),
        };
        let mut handle = unsafe {
            self.device
                .get_handle()
                .allocate_descriptor_sets(&alloc_info)?
        };
        Ok(handle.pop().unwrap())
    }
}

impl Destructible for DescriptorPool {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        trace!("Destroyed VkDescriptorPool {:p}", self.handle);

        unsafe {
            self.device
                .get_handle()
                .destroy_descriptor_pool(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl Drop for DescriptorPool {
    fn drop(&mut self) {
        self.destroy();
    }
}
