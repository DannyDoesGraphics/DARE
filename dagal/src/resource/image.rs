use std::ptr;

use anyhow::Result;
use ash::prelude::VkResult;
use ash::vk;
use ash::vk::Handle;

use crate::allocators::{Allocator, ArcAllocation, ArcAllocator, GPUAllocatorImpl};
use crate::command::command_buffer::CmdBuffer;
use crate::resource::traits::{Nameable, Resource};
use crate::traits::{AsRaw, Destructible};

#[derive(Debug)]
pub struct Image<A: Allocator = GPUAllocatorImpl> {
    handle: vk::Image,
    format: vk::Format,
    extent: vk::Extent3D,
    mip_levels: u32,
    usage_flags: vk::ImageUsageFlags,
    image_type: vk::ImageType,
    device: crate::device::LogicalDevice,
    allocation: Option<ArcAllocation<A>>,
    image_managed: bool,
}

pub enum ImageCreateInfo<'a, A: Allocator = GPUAllocatorImpl> {
    /// Create a new image from an existing VkImage whose memory is not managed by the application
    /// (i.e. swapchain images)
    FromVkNotManaged {
        device: crate::device::LogicalDevice,
        image: vk::Image,
        format: vk::Format,
        extent: vk::Extent3D,
        mip_levels: u32,
        usage_flags: vk::ImageUsageFlags,
        image_type: vk::ImageType,
        name: Option<&'a str>,
    },
    /// Create a new image without any allocation made
    NewUnallocated {
        device: crate::device::LogicalDevice,
        image_ci: vk::ImageCreateInfo<'a>,
        name: Option<&'a str>,
    },
    /// Create a new image that has allocated memory
    NewAllocated {
        device: crate::device::LogicalDevice,
        allocator: &'a mut ArcAllocator<A>,
        location: crate::allocators::MemoryLocation,
        image_ci: vk::ImageCreateInfo<'a>,
        name: Option<&'a str>,
    },
}

impl<A: Allocator> Image<A> {
    /// Get all used usage flags
    pub fn usage_flags(&self) -> vk::ImageUsageFlags {
        self.usage_flags
    }

    /// Acquire image format
    pub fn format(&self) -> vk::Format {
        self.format
    }

    /// Acquire image extent
    pub fn extent(&self) -> vk::Extent3D {
        self.extent
    }

    /// Acquire image mip levels
    pub fn mip_levels(&self) -> u32 {
        self.mip_levels
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

    /// Copies the passed image into the current image
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
            src_image: unsafe { *image.as_raw() },
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

    /// Acquires a full image view
    pub fn acquire_full_image_view(&self) -> VkResult<vk::ImageView> {
        let aspect_flag: vk::ImageAspectFlags = if self.usage_flags
            & vk::ImageUsageFlags::COLOR_ATTACHMENT
            == vk::ImageUsageFlags::COLOR_ATTACHMENT
        {
            vk::ImageAspectFlags::COLOR
        } else if self.usage_flags & vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
            == vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
        {
            vk::ImageAspectFlags::DEPTH
        } else {
            unimplemented!()
        };
        unsafe {
            self.device.get_handle().create_image_view(
                &vk::ImageViewCreateInfo {
                    s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::ImageViewCreateFlags::empty(),
                    image: self.handle,
                    view_type: if self.image_type == vk::ImageType::TYPE_2D {
                        vk::ImageViewType::TYPE_2D
                    } else if self.image_type == vk::ImageType::TYPE_1D {
                        vk::ImageViewType::TYPE_1D
                    } else if self.image_type == vk::ImageType::TYPE_3D {
                        vk::ImageViewType::TYPE_3D
                    } else {
                        unimplemented!()
                    },
                    format: self.format,
                    components: Default::default(),
                    subresource_range: Image::image_subresource_range(aspect_flag),
                    _marker: Default::default(),
                },
                None,
            )
        }
    }
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

impl<'a, A: Allocator + 'a> Resource<'a> for Image<A> {
    type CreateInfo = ImageCreateInfo<'a, A>;

    ///
    /// # Examples
    /// Creating an empty image with no allocation
    /// ```
    /// use std::ptr;
    /// use ash::vk;
    /// use dagal::resource::traits::Resource;
    /// use dagal::util::tests::TestSettings;
    /// let test_vulkan = dagal::util::tests::create_vulkan_and_device(TestSettings::default());
    /// let image: dagal::resource::Image = dagal::resource::Image::new(dagal::resource::ImageCreateInfo::NewUnallocated {
    ///     device: test_vulkan.device.as_ref().unwrap().clone(),
    ///     image_ci:vk::ImageCreateInfo {
    ///         s_type: vk::StructureType::IMAGE_CREATE_INFO,
    ///         p_next: ptr::null(),
    ///         flags: vk::ImageCreateFlags::empty(),
    ///         image_type: vk::ImageType::TYPE_2D,
    ///         format: vk::Format::R8G8B8A8_SRGB,
    ///         extent: vk::Extent3D {
    ///             width: 10,
    ///             height: 10,
    ///             depth: 1,
    ///         },
    ///         mip_levels: 1,
    ///         array_layers: 1,
    ///         samples: vk::SampleCountFlags::TYPE_1,
    ///         tiling: vk::ImageTiling::LINEAR,
    ///         usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
    ///         sharing_mode: vk::SharingMode::EXCLUSIVE,
    ///         queue_family_index_count: 1,
    ///         p_queue_family_indices: &test_vulkan.compute_queue.as_ref().unwrap().get_family_index(),
    ///         initial_layout: vk::ImageLayout::UNDEFINED,
    ///         _marker: Default::default(),
    ///     },
    ///     name: None,
    /// }).unwrap();
    /// drop(image);
    /// ```
    /// Creating an image with an allocation
    /// ```
    /// use std::ptr;
    /// use ash::vk;
    /// use dagal::allocators::GPUAllocatorImpl;
    /// use dagal::resource::traits::Resource;
    /// use dagal::util::tests::TestSettings;
    /// use dagal::gpu_allocator;
    /// let test_vulkan = dagal::util::tests::create_vulkan_and_device(TestSettings::default());
    /// let allocator = GPUAllocatorImpl::new(gpu_allocator::vulkan::AllocatorCreateDesc {
    ///     instance: test_vulkan.instance.get_instance().clone(),
    ///     device: test_vulkan.device.as_ref().unwrap().get_handle().clone(),
    ///     physical_device: test_vulkan.physical_device.as_ref().unwrap().handle().clone(),
    ///     debug_settings: gpu_allocator::AllocatorDebugSettings {
    ///         log_memory_information: false,
    ///             log_leaks_on_shutdown: true,
    ///             store_stack_traces: false,
    ///             log_allocations: false,
    ///             log_frees: false,
    ///             log_stack_traces: false,
    ///         },
    ///         buffer_device_address: false,
    ///         allocation_sizes: Default::default(),
    ///  }).unwrap();
    /// let mut allocator = dagal::allocators::ArcAllocator::new(allocator);
    /// let image: dagal::resource::Image = dagal::resource::Image::new(dagal::resource::ImageCreateInfo::NewAllocated {
    ///     device: test_vulkan.device.as_ref().unwrap().clone(),
    ///     image_ci: vk::ImageCreateInfo {
    ///         s_type: vk::StructureType::IMAGE_CREATE_INFO,
    ///         p_next: ptr::null(),
    ///         flags: vk::ImageCreateFlags::empty(),
    ///         image_type: vk::ImageType::TYPE_2D,
    ///         format: vk::Format::R8G8B8A8_SRGB,
    ///         extent: vk::Extent3D {
    ///             width: 10,
    ///             height: 10,
    ///             depth: 1,
    ///         },
    ///         mip_levels: 1,
    ///         array_layers: 1,
    ///         samples: vk::SampleCountFlags::TYPE_1,
    ///         tiling: vk::ImageTiling::LINEAR,
    ///         usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
    ///         sharing_mode: vk::SharingMode::EXCLUSIVE,
    ///         queue_family_index_count: 1,
    ///         p_queue_family_indices: &test_vulkan.compute_queue.as_ref().unwrap().get_family_index(),
    ///         initial_layout: vk::ImageLayout::UNDEFINED,
    ///         _marker: Default::default(),
    ///     },
    ///     allocator: &mut allocator,
    ///     location: dagal::allocators::MemoryLocation::GpuOnly,
    ///     name: None,
    /// }).unwrap();
    /// drop(image);
    /// ```
    fn new(create_info: ImageCreateInfo<'a, A>) -> Result<Self>
           where
               Self: Sized,
    {
        match create_info {
            ImageCreateInfo::FromVkNotManaged {
                device,
                image,
                usage_flags,
                image_type,
                mip_levels,
                format,
                extent,
                name,
            } => {
                let mut res = Self {
                    device,
                    handle: image,
                    format,
                    extent,
                    mip_levels,
                    usage_flags,
                    image_type,
                    allocation: None,
                    image_managed: false,
                };
                crate::resource::traits::update_name(&mut res, name).unwrap_or(Ok(()))?;
                Ok(res)
            }
            ImageCreateInfo::NewUnallocated {
                device,
                image_ci,
                name,
            } => {
                let handle = unsafe { device.get_handle().create_image(&image_ci, None)? };
                #[cfg(feature = "log-lifetimes")]
                tracing::trace!("Created VkImage {:p}", handle);

                let mut handle = Self {
                    handle,
                    format: image_ci.format,
                    extent: image_ci.extent,
                    mip_levels: image_ci.mip_levels,
                    usage_flags: image_ci.usage,
                    image_type: image_ci.image_type,
                    device,
                    allocation: None,
                    image_managed: true,
                };
                crate::resource::traits::update_name(&mut handle, name).unwrap_or(Ok(()))?;

                Ok(handle)
            }
            ImageCreateInfo::NewAllocated {
                device,
                allocator,
                location,
                image_ci,
                name,
            } => {
                let mut image = Self::new(ImageCreateInfo::NewUnallocated {
                    device,
                    image_ci,
                    name,
                })?;
                let memory_requirements = unsafe {
                    image
                        .device
                        .get_handle()
                        .get_image_memory_requirements(image.handle)
                };
                let allocation =
                    allocator.allocate(name.unwrap_or(""), &memory_requirements, location)?;
                unsafe {
                    image.device.get_handle().bind_image_memory(
                        image.handle,
                        allocation.memory()?,
                        allocation.offset()?,
                    )?
                }
                image.allocation = Some(allocation);
                Ok(image)
            }
        }
    }

    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }
}

impl<A: Allocator> AsRaw for Image<A> {
    type RawType = vk::Image;

    unsafe fn as_raw(&self) -> &Self::RawType {
        &self.handle
    }

    unsafe fn as_raw_mut(&mut self) -> &mut Self::RawType {
        &mut self.handle
    }

    unsafe fn raw(self) -> Self::RawType {
        self.handle
    }
}

impl<A: Allocator> Nameable for Image<A> {
    const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::IMAGE;
    fn set_name(
        &mut self,
        debug_utils: &ash::ext::debug_utils::Device,
        name: &str,
    ) -> Result<()> {
        crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
        Ok(())
    }
}

impl<A: Allocator> Destructible for Image<A> {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Destroying VkImage {:p}", self.handle);

        if let Some(mut allocation) = self.allocation.take() {
            allocation.destroy();
            drop(allocation);
        }
        if self.image_managed {
            unsafe {
                self.device.get_handle().destroy_image(self.handle, None);
            }
        }
    }
}

#[cfg(feature = "raii")]
impl<A: Allocator> Drop for Image<A> {
    fn drop(&mut self) {
        self.destroy();
    }
}
