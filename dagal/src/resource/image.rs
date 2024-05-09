use crate::command::command_buffer::CmdBuffer;
use crate::traits::Destructible;
use anyhow::Result;
use ash::vk;
use std::ptr;
use tracing::trace;
use crate::allocators::Allocation;

pub struct Image<A: crate::allocators::Allocator = crate::allocators::vk_mem_impl::VkMemAllocator> {
    handle: vk::Image,
    format: vk::Format,
    extent: vk::Extent3D,
    device: crate::device::LogicalDevice,
    allocation: Option<A::Allocation>,
}

impl Image {
    pub fn image_subresource_range(aspect: vk::ImageAspectFlags) -> vk::ImageSubresourceRange {
        vk::ImageSubresourceRange {
            aspect_mask: aspect,
            base_mip_level: 0,
            level_count: vk::REMAINING_MIP_LEVELS,
            base_array_layer: 0,
            layer_count: vk::REMAINING_ARRAY_LAYERS,
        }
    }
}

impl Image {
    /// Create an [`Image`] from [`VkImage`](vk::Image)
    pub fn from_vk(
        device: crate::device::LogicalDevice,
        image: vk::Image,
        format: vk::Format,
        extent: vk::Extent3D,
    ) -> Self {
        #[cfg(feature = "log-lifetimes")]
        trace!("Creating VkImage from Vk {:p}", image);

        Self {
            handle: image,
            format,
            extent,
            device,
            allocation: None,
        }
    }

    /// Transitions an image from one layout to another layout
    pub fn transition(
        &self,
        cmd: &crate::command::CommandBufferRecording,
        queue: &crate::device::Queue,
        current_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) {
        let image_barrier = vk::ImageMemoryBarrier2 {
            s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
            p_next: ptr::null(),
            src_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
            src_access_mask: vk::AccessFlags2::MEMORY_WRITE,
            dst_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
            dst_access_mask: vk::AccessFlags2::MEMORY_WRITE | vk::AccessFlags2::MEMORY_READ,
            old_layout: current_layout,
            new_layout,
            src_queue_family_index: queue.get_family_index(),
            dst_queue_family_index: queue.get_family_index(),
            image: self.handle,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: if new_layout == vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL {
                    vk::ImageAspectFlags::DEPTH
                } else {
                    vk::ImageAspectFlags::COLOR
                },
                base_mip_level: 0,
                level_count: vk::REMAINING_MIP_LEVELS,
                base_array_layer: 0,
                layer_count: vk::REMAINING_ARRAY_LAYERS,
            },
            _marker: Default::default(),
        };
        let dependency_info = vk::DependencyInfo {
            s_type: vk::StructureType::DEPENDENCY_INFO,
            p_next: ptr::null(),
            dependency_flags: vk::DependencyFlags::empty(),
            memory_barrier_count: 0,
            p_memory_barriers: ptr::null(),
            buffer_memory_barrier_count: 0,
            p_buffer_memory_barriers: ptr::null(),
            image_memory_barrier_count: 1,
            p_image_memory_barriers: &image_barrier,
            _marker: Default::default(),
        };
        unsafe {
            self.device
                .get_handle()
                .cmd_pipeline_barrier2(cmd.handle(), &dependency_info);
        }
    }

    /// Copies image passed in to current image
    pub fn copy_from(&self, cmd: &crate::command::CommandBufferRecording, image: &Image) {
        let from_extent: vk::Extent3D = image.extent;
        let blit_region = vk::ImageBlit2 {
            s_type: vk::StructureType::IMAGE_BLIT_2,
            p_next: ptr::null(),
            src_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            src_offsets: [
                vk::Offset3D { x: 0, y: 0, z: 0 },
                vk::Offset3D {
                    x: from_extent.width as i32,
                    y: from_extent.height as i32,
                    z: from_extent.depth as i32,
                },
            ],
            dst_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            dst_offsets: [
                vk::Offset3D { x: 0, y: 0, z: 0 },
                vk::Offset3D {
                    x: self.extent.width as i32,
                    y: self.extent.height as i32,
                    z: self.extent.depth as i32,
                },
            ],
            _marker: Default::default(),
        };
        let blint_info = vk::BlitImageInfo2 {
            s_type: vk::StructureType::BLIT_IMAGE_INFO_2,
            p_next: ptr::null(),
            src_image: image.handle(),
            src_image_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            dst_image: self.handle,
            dst_image_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            region_count: 1,
            p_regions: &blit_region,
            filter: Default::default(),
            _marker: Default::default(),
        };
        unsafe {
            self.device
                .get_handle()
                .cmd_blit_image2(cmd.handle(), &blint_info);
        }
    }

    pub fn handle(&self) -> vk::Image {
        self.handle
    }

    /// Get the image extents
    pub fn extent(&self) -> vk::Extent3D {
        self.extent
    }

    /// Get the image format
    pub fn format(&self) -> vk::Format {
        self.format
    }
}

impl<A: crate::allocators::Allocator> Image<A> {
    /// Create a new image with no memory bounded
    pub fn new_empty(
        device: crate::device::LogicalDevice,
        image_ci: &vk::ImageCreateInfo,
    ) -> Result<Self> {
        let handle: vk::Image = unsafe { device.get_handle().create_image(image_ci, None)? };

        #[cfg(feature = "log-lifetimes")]
        trace!("Creating VkImage with no memory {:p}", handle);

        Ok(Self {
            handle,
            format: image_ci.format,
            extent: image_ci.extent,
            device,
            allocation: None,
        })
    }

    /// Binds data to already existing memory
    pub fn new_with_memory(
        device: crate::device::LogicalDevice,
        image_ci: &vk::ImageCreateInfo,
        allocation: A::Allocation,
    ) -> Result<Self> {
        let mut handle = Self::new_empty(device, image_ci)?;
        handle.allocation = Some(allocation);

        #[cfg(feature = "log-lifetimes")]
        trace!("Creating VkImage with existing memory {:p}", handle.handle);

        Ok(handle)
    }

    /// Allocates a new image with new memory allocated for it
    pub fn new_with_new_memory(
        device: crate::device::LogicalDevice,
        image_ci: &vk::ImageCreateInfo,
        allocator: &mut A, 
        memory_type: crate::allocators::MemoryType,
        name: &str,
    ) -> Result<Self> {
        let mut handle = Self::new_empty(device.clone(), image_ci)?;

        let memory_requirements = unsafe {
            device
                .get_handle()
                .get_image_memory_requirements(handle.handle)
        };
        let allocation = allocator.allocate(name, &memory_requirements, memory_type)?;
        unsafe {
            device.get_handle().bind_image_memory(
                handle.handle,
                allocation.memory(),
                allocation.offset(),
            )?
        }
        handle.allocation = Some(allocation);

        #[cfg(feature = "log-lifetimes")]
        trace!("Creating VkImage with new memory {:p}", handle.handle);

        Ok(handle)
    }
}

#[cfg(feature = "vk-mem-rs")]
impl Image<crate::allocators::VkMemAllocator> {
    
}

#[cfg(feature = "gpu-allocator")]
impl Image<crate::allocators::GpuAllocator> {
}


impl Destructible for Image {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        trace!("Destroying VkImage {:p}", self.handle);

        if let Some(mut allocation) = self.allocation.take() {
            allocation.destroy();
            drop(allocation);
        }
        unsafe {
            self.device.get_handle().destroy_image(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl<A: crate::allocators::Allocator> Drop for Image<A> {
    fn drop(&mut self) {
        self.destroy();
    }
}
