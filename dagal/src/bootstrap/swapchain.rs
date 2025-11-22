use std::collections::HashSet;
use std::fmt::Debug;
use std::ptr;

use anyhow::Result;
use ash::vk;
use derivative::Derivative;

/// A builder pattern struct to make creating [`dagal::wsi::Swapchain`](crate::wsi::Swapchain)
/// easier such as automating priority and error handling.
///
/// # Image format / Color space / Present mode picking
/// Fundamentally, they all work the same image formats / color spaces / present
/// modes which are inserted first, have the highest priority (FIFO). Effectively this is just
/// a priority queue which terminates when it finds the first available preference.
///
/// # Concurrent/Exclusive
/// If all queues passed to the builder (i.e. [`push_queues`](SwapchainBuilder::push_queues)) all
/// have the same family index and queue index, it will automatically use exclusive as the image
/// sharing mode.
///
/// # Examples
/// ```
/// use ash::vk;
/// use ash_window;
/// use raw_window_handle::HasDisplayHandle;
/// use dagal::util::tests::TestSettings;
/// let test_app = dagal::util::tests::TestApp::<winit::window::Window>::new();
/// test_app.attach_function(|window: &winit::window::Window| {
///     let test_vulkan = dagal::util::tests::create_vulkan_and_device(
/// 		TestSettings::from_rdh(window.display_handle().unwrap().as_raw()).add_physical_device_extension(ash::khr::swapchain::NAME.as_ptr())
///         .add_physical_device_extension(ash::khr::swapchain::NAME.as_ptr())
/// 	);
///     let mut surface: dagal::wsi::Surface = dagal::wsi::Surface::new(test_vulkan.instance.get_entry(), test_vulkan.instance.get_instance(), window).unwrap();
///     let surface = surface.query_details(test_vulkan.physical_device.as_ref().unwrap().handle()).unwrap();
///     let swapchain = dagal::bootstrap::SwapchainBuilder::new(&surface)
/// 	.request_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
/// 	.request_image_format(vk::Format::R8G8B8A8_SRGB)
/// 	.request_present_mode(vk::PresentModeKHR::MAILBOX) // Tries to find mailbox first
///     .request_present_mode(vk::PresentModeKHR::FIFO) // If not, falls back to FIFO (Hence, FIFO)
///     .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
///     .query_extent_from_window(window)
/// 	.build(test_vulkan.instance.get_instance(), test_vulkan.device.as_ref().unwrap().clone()).unwrap();
///     drop(swapchain);
///     drop(surface);
///     drop(test_vulkan);
/// }).run();
/// ```
#[derive(Derivative)]
#[derivative(Debug)]
pub struct SwapchainBuilder<'a> {
    surface_queried: &'a crate::wsi::SurfaceQueried,

    preferred_image_formats: Vec<vk::Format>,
    preferred_present_modes: Vec<vk::PresentModeKHR>,
    preferred_color_spaces: Vec<vk::ColorSpaceKHR>,

    family_indices: HashSet<u32>,
    image_usage: vk::ImageUsageFlags,
    image_extent: vk::Extent2D,

    preferred_image_counts: u32,
}

impl<'a> SwapchainBuilder<'a> {
    pub fn new(surface: &'a crate::wsi::SurfaceQueried) -> Self {
        Self {
            surface_queried: surface,
            preferred_image_formats: Vec::new(),
            family_indices: HashSet::new(),
            image_usage: vk::ImageUsageFlags::empty(),
            image_extent: vk::Extent2D::default(),
            preferred_color_spaces: vec![],
            preferred_present_modes: vec![],
            preferred_image_counts: 0,
        }
    }

    /// Adds an image format to search for in the swapchain.
    ///
    /// **Functions like a queue meaning the first images in, get the highest priority.**
    pub fn request_image_format(mut self, format: vk::Format) -> Self {
        self.preferred_image_formats.push(format);
        self
    }

    /// Adds a present mode to search in the swapchain to use.
    pub fn request_present_mode(mut self, present: vk::PresentModeKHR) -> Self {
        self.preferred_present_modes.push(present);
        self
    }

    /// Adds a color format for the swapchain to use.
    pub fn request_color_space(mut self, color: vk::ColorSpaceKHR) -> Self {
        self.preferred_color_spaces.push(color);
        self
    }

    /// Adds a queue which is expected to use [`VkSwapchainKHR`](vk::SwapchainKHR).
    pub fn push_queue(mut self, queue: &crate::device::Queue) -> Self {
        self.family_indices.insert(queue.get_family_index());
        self
    }

    /// If you're using [`VkQueue`](vk::Queue), then you can manually push the family indices
    pub fn push_family_queue_index(mut self, family_queue_index: u32) -> Self {
        self.family_indices.insert(family_queue_index);
        self
    }

    /// Clamps extent to the surface capabilities
    pub fn clamp_extent(&self, extent: &vk::Extent2D) -> vk::Extent2D {
        let surface_capabilities = self.surface_queried.get_capabilities();
        vk::Extent2D {
            width: extent.width.clamp(
                surface_capabilities.min_image_extent.width,
                surface_capabilities.max_image_extent.width,
            ),
            height: extent.height.clamp(
                surface_capabilities.min_image_extent.height,
                surface_capabilities.max_image_extent.height,
            ),
        }
    }

    /// Set swapchain image extents
    pub fn set_extent(mut self, extent: vk::Extent2D) -> Self {
        let surface_capabilities = self.surface_queried.get_capabilities();
        assert!(
            surface_capabilities.min_image_extent.width <= extent.width,
            "{} <= {}",
            surface_capabilities.min_image_extent.width,
            extent.width
        );
        assert!(
            surface_capabilities.max_image_extent.width >= extent.width,
            "{} >= {}",
            surface_capabilities.max_image_extent.width,
            extent.width
        );
        assert!(
            surface_capabilities.min_image_extent.height <= extent.height,
            "{} <= {}",
            surface_capabilities.min_image_extent.height,
            extent.height
        );
        assert!(
            surface_capabilities.max_image_extent.height >= extent.height,
            "{} >= {}",
            surface_capabilities.max_image_extent.height,
            extent.height
        );
        self.image_extent = extent;
        self
    }

    /// Queries a window for it's width
    pub fn query_extent_from_window<T: crate::wsi::DagalWindow>(mut self, window: &T) -> Self {
        let surface_capabilities = self.surface_queried.get_capabilities();
        self.image_extent = vk::Extent2D {
            width: window.width().clamp(
                surface_capabilities.min_image_extent.width,
                surface_capabilities.max_image_extent.width,
            ),
            height: window.height().clamp(
                surface_capabilities.min_image_extent.height,
                surface_capabilities.max_image_extent.height,
            ),
        };
        self
    }

    /// Set image usage
    pub fn image_usage(mut self, usage: vk::ImageUsageFlags) -> Self {
        self.image_usage |= usage;
        self
    }

    /// Finds first occurrence in an element in a: Vec<T> in B: Vec<T> and clones it and returns it
    fn find_first_occurrence<T: Clone + PartialEq + Debug>(a: &[T], b: &[T]) -> Option<T> {
        for a in a.iter() {
            if b.contains(a) {
                return Some(a.clone());
            }
        }
        None
    }

    /// Sets the preferred image counts of a swapchain.
    ///
    /// [`None`] and 0 represents using the minimum amount from [`VkSurfaceCapabilitiesKHR`](vk::SurfaceCapabilitiesKHR)
    pub fn min_image_count(mut self, preferred_count: Option<u32>) -> Self {
        if let Some(preferred_count) = preferred_count {
            let surface_capabilities = self.surface_queried.get_capabilities();
            assert!(preferred_count >= surface_capabilities.min_image_count);
            assert!(preferred_count <= surface_capabilities.max_image_count)
        }
        self.preferred_image_counts = preferred_count.unwrap_or(0);
        self
    }

    /// Builds the swapchain
    pub fn build(
        self,
        instance: &ash::Instance,
        device: crate::device::LogicalDevice,
    ) -> crate::Result<crate::wsi::Swapchain> {
        let queue_family_indices: Vec<u32> = self.family_indices.iter().copied().collect();
        let surface_capabilities = self.surface_queried.get_capabilities();

        // Get available formats from surface
        let available_formats: Vec<vk::Format> = self
            .surface_queried
            .get_formats()
            .iter()
            .map(|format| format.format)
            .collect();

        // Get available color spaces from surface
        let available_color_spaces: Vec<vk::ColorSpaceKHR> = self
            .surface_queried
            .get_formats()
            .iter()
            .map(|format| format.color_space)
            .collect();

        let swapchain_ci = vk::SwapchainCreateInfoKHR {
            s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
            p_next: ptr::null(),
            flags: vk::SwapchainCreateFlagsKHR::empty(),
            surface: self.surface_queried.handle(),
            min_image_count: if self.preferred_image_counts == 0 {
                surface_capabilities.min_image_count
            } else {
                self.preferred_image_counts
            },
            image_format: Self::find_first_occurrence(
                self.preferred_image_formats.as_slice(),
                available_formats.as_slice(),
            )
            .unwrap(),
            image_color_space: Self::find_first_occurrence(
                self.preferred_color_spaces.as_slice(),
                available_color_spaces.as_slice(),
            )
            .unwrap(),
            image_extent: self.image_extent,
            image_array_layers: 1,
            image_usage: self.image_usage,
            image_sharing_mode: if self.family_indices.len() > 1 {
                vk::SharingMode::CONCURRENT
            } else {
                vk::SharingMode::EXCLUSIVE
            },
            queue_family_index_count: if self.family_indices.len() <= 1 {
                0
            } else {
                self.family_indices.len() as u32
            },
            p_queue_family_indices: queue_family_indices.as_ptr(),
            pre_transform: surface_capabilities.current_transform,
            composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
            present_mode: Self::find_first_occurrence(
                self.preferred_present_modes.as_slice(),
                self.surface_queried.get_present_modes(),
            )
            .unwrap(),
            clipped: vk::TRUE,
            old_swapchain: vk::SwapchainKHR::null(),
            _marker: Default::default(),
        };
        crate::wsi::Swapchain::new(instance, device, &swapchain_ci)
    }
}
