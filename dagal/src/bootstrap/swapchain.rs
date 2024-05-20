use anyhow::Result;
use ash::vk;
use derivative::Derivative;
use std::collections::HashSet;
use std::fmt::Debug;
use std::ptr;

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
///     let (instance, physical_device, logical_device, _, mut stack) = dagal::util::tests::create_vulkan_and_device(
/// 		TestSettings::from_rdh(window.display_handle().unwrap().as_raw()).add_physical_device_extension(ash::khr::swapchain::NAME.as_ptr())
///         .add_physical_device_extension(ash::khr::swapchain::NAME.as_ptr())
/// 	);
///     let mut surface: dagal::wsi::Surface = dagal::wsi::Surface::new(instance.get_entry(), instance.get_instance(), window).unwrap();
///     surface.query_details(physical_device.handle()).unwrap();
///     let swapchain = dagal::bootstrap::SwapchainBuilder::new(&surface)
/// 	.request_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
/// 	.request_image_format(vk::Format::R8G8B8A8_SRGB)
/// 	.request_present_mode(vk::PresentModeKHR::MAILBOX) // Tries to find mailbox first
///     .request_present_mode(vk::PresentModeKHR::FIFO) // If not, falls back to FIFO (Hence, FIFO)
///     .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
///     .query_extent_from_window(window)
/// 	.build(instance.get_instance(), logical_device.clone());
///     stack.push(move || {
///         drop(surface);
/// 	});
///     stack.push(move || {
///         drop(swapchain);
/// 	});
///     stack.flush();
/// }).run();
/// ```
#[derive(Derivative)]
#[derivative(Debug)]
pub struct SwapchainBuilder {
    surface: vk::SurfaceKHR,
    surface_capabilities: vk::SurfaceCapabilitiesKHR,

    image_formats: Vec<vk::Format>,
    preferred_image_formats: Vec<vk::Format>,

    present_modes: Vec<vk::PresentModeKHR>,
    preferred_present_modes: Vec<vk::PresentModeKHR>,

    preferred_color_spaces: Vec<vk::ColorSpaceKHR>,
    color_spaces: Vec<vk::ColorSpaceKHR>,

    family_indices: HashSet<u32>,
    image_usage: vk::ImageUsageFlags,
    image_extent: vk::Extent2D,

    preferred_image_counts: u32,
}

impl SwapchainBuilder {
    pub fn new(surface: &crate::wsi::Surface) -> Self {
        Self {
            surface: surface.handle(),
            surface_capabilities: surface.get_capabilities().unwrap(),
            image_formats: surface
                .get_formats()
                .as_ref()
                .unwrap()
                .iter()
                .map(|format| format.format)
                .collect(),
            preferred_image_formats: Vec::new(),
            present_modes: surface.get_present_modes().as_ref().unwrap().to_vec(),
            family_indices: HashSet::new(),
            image_usage: vk::ImageUsageFlags::empty(),
            image_extent: vk::Extent2D::default(),
            color_spaces: surface
                .get_formats()
                .as_ref()
                .unwrap()
                .iter()
                .map(|format| format.color_space)
                .collect(),
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

    /// Set swapchain image extents
    pub fn set_extent(mut self, extent: vk::Extent2D) -> Self {
        assert!(
            self.surface_capabilities.min_image_extent.width <= extent.width
                && extent.width <= self.surface_capabilities.max_image_extent.width
        );
        assert!(
            self.surface_capabilities.min_image_extent.height <= extent.height
                && extent.height <= self.surface_capabilities.max_image_extent.height
        );
        self.image_extent = extent;
        self
    }

    /// Queries a window for it's width
    pub fn query_extent_from_window<T: crate::wsi::DagalWindow>(mut self, window: &T) -> Self {
        self.image_extent = vk::Extent2D {
            width: window.width().clamp(
                self.surface_capabilities.min_image_extent.width,
                self.surface_capabilities.max_image_extent.width,
            ),
            height: window.height().clamp(
                self.surface_capabilities.min_image_extent.height,
                self.surface_capabilities.max_image_extent.height,
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
            assert!(preferred_count > self.surface_capabilities.min_image_count);
        }
        self.preferred_image_counts = preferred_count.unwrap_or(0);
        self
    }

    /// Builds the swapchain
    pub fn build(
        self,
        instance: &ash::Instance,
        device: crate::device::LogicalDevice,
    ) -> Result<crate::wsi::Swapchain> {
        let queue_family_indices: Vec<u32> = self.family_indices.iter().copied().collect();
        let swapchain_ci = vk::SwapchainCreateInfoKHR {
            s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
            p_next: ptr::null(),
            flags: vk::SwapchainCreateFlagsKHR::empty(),
            surface: self.surface,
            min_image_count: if self.preferred_image_counts == 0 { self.surface_capabilities.min_image_count } else { self.preferred_image_counts },
            image_format: Self::find_first_occurrence(
                self.preferred_image_formats.as_slice(),
                self.image_formats.as_slice(),
            )
            .unwrap(),
            image_color_space: Self::find_first_occurrence(
                self.preferred_color_spaces.as_slice(),
                self.color_spaces.as_slice(),
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
            pre_transform: self.surface_capabilities.current_transform,
            composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
            present_mode: Self::find_first_occurrence(
                self.preferred_present_modes.as_slice(),
                self.present_modes.as_slice(),
            )
            .unwrap(),
            clipped: vk::TRUE,
            old_swapchain: vk::SwapchainKHR::null(),
            _marker: Default::default(),
        };
        crate::wsi::Swapchain::new(instance, device, &swapchain_ci)
    }
}
