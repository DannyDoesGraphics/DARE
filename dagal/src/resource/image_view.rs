use crate::allocators::Allocator;
use crate::resource::traits::{Nameable, Resource};
use crate::traits::{AsRaw, Destructible};
use anyhow::Result;
use ash::vk;
use ash::vk::Handle;

#[derive(Debug)]
pub struct ImageView {
    handle: vk::ImageView,
    device: crate::device::LogicalDevice,
    name: Option<String>,
}
unsafe impl Send for ImageView {}

impl PartialEq for ImageView {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}

impl Destructible for ImageView {
    fn destroy(&mut self) {
        unsafe {
            self.device
                .get_handle()
                .destroy_image_view(self.handle, None);
        }
    }
}

pub enum ImageViewCreateInfo<'a> {
    /// Create a VkImageView from a [`VkImageViewCreateInfo`](vk::ImageViewCreateInfo) struct
    /// # Example
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
    /// let image_view = dagal::resource::ImageView::new(dagal::resource::ImageViewCreateInfo::FromCreateInfo {
    ///     create_info: vk::ImageViewCreateInfo {
    ///         s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
    ///         p_next: ptr::null(),
    ///         flags: vk::ImageViewCreateFlags::empty(),
    ///         image: image.handle(),
    ///         view_type: vk::ImageViewType::TYPE_2D,
    ///         format: image.format(),
    ///         components: vk::ComponentMapping::default(),
    ///         subresource_range: dagal::resource::Image::image_subresource_range(vk::ImageAspectFlags::COLOR),
    ///         _marker: Default::default(),
    ///     },
    ///     device: test_vulkan.device.as_ref().unwrap().clone(),
    /// }).unwrap();
    /// drop(image_view);
    /// drop(image);
    /// ```
    FromCreateInfo {
        device: crate::device::LogicalDevice,
        create_info: vk::ImageViewCreateInfo<'a>,
    },
    /// Create a VkImageView from an existing one
    FromVk {
        device: crate::device::LogicalDevice,
        image_view: vk::ImageView,
    },
}

impl Resource for ImageView {
    type CreateInfo<'a> = ImageViewCreateInfo<'a>;
    fn new(create_info: ImageViewCreateInfo) -> Result<Self>
    where
        Self: Sized,
    {
        match create_info {
            ImageViewCreateInfo::FromCreateInfo {
                device,
                create_info,
            } => {
                let handle = unsafe { device.get_handle().create_image_view(&create_info, None)? };
                Ok(Self {
                    handle,
                    device,
                    name: None,
                })
            }
            ImageViewCreateInfo::FromVk { device, image_view } => Ok(Self {
                handle: image_view,
                device,
                name: None,
            }),
        }
    }

    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }
}

impl AsRaw for ImageView {
    type RawType = vk::ImageView;

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

impl Nameable for ImageView {
    const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::IMAGE_VIEW;
    fn set_name(
        &mut self,
        debug_utils: &ash::ext::debug_utils::Device,
        name: &str,
    ) -> anyhow::Result<()> {
        crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
        self.name = Some(name.to_string());
        Ok(())
    }
}

#[cfg(feature = "raii")]
impl Drop for ImageView {
    fn drop(&mut self) {
        self.destroy();
    }
}
