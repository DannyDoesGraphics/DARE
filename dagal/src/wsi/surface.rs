use crate::traits::Destructible;
use anyhow::Result;
use ash;
use ash::vk;
use derivative::Derivative;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tracing::trace;

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct Surface {
    handle: vk::SurfaceKHR,
    capabilities: Option<vk::SurfaceCapabilitiesKHR>,
    formats: Option<Vec<vk::SurfaceFormatKHR>>,
    present_modes: Option<Vec<vk::PresentModeKHR>>,
    #[derivative(Debug = "ignore")]
    ext: ash::khr::surface::Instance,
}

impl Surface {
    /// Construct a new [`Surface`] **without** any present modes, capabilities, and formats.
    /// See [`Surface::get_capabilities`] to determine such.
    /// # Examples
    /// ```
    /// use crate::dagal;
    /// use dagal::ash::vk;
    /// use raw_window_handle::HasDisplayHandle;
    /// use dagal::util::tests::TestSettings;
    /// use dagal::traits::*;
    /// // The TestApp is purely use for us to properly acquire and clean a test window. However,
    /// // this can apply for any window which has the HasDisplayHandle + HasWindowHandle traits
    /// // which in this case is winit, but could be any other window manager.
    /// let test_app = dagal::util::tests::TestApp::<winit::window::Window>::new();
    /// test_app.attach_function(|window: &winit::window::Window | {
    ///     let (instance, physical_device, _, _, mut stack) = dagal::util::tests::create_vulkan_and_device(
    ///         TestSettings::from_rdh(window.display_handle().unwrap().as_raw())
    /// 	);
    ///     // Construct a surface
    ///     let mut surface: dagal::wsi::Surface = dagal::wsi::Surface::new(instance.get_entry(), instance.get_instance(), window).unwrap();
    ///     surface.query_details(physical_device.handle()).unwrap();
    ///     assert!(surface.get_capabilities().as_ref().unwrap().min_image_count > 0);
    ///     assert!(surface.get_formats().as_ref().unwrap().len() > 0);
    ///     assert!(surface.get_present_modes().as_ref().unwrap().len() > 0);
    ///     stack.push(move || {
    ///         surface.destroy();
    /// 	});
    ///     stack.flush();
    /// }).run();
    /// ```
    pub fn new<T>(entry: &ash::Entry, instance: &ash::Instance, window: &T) -> Result<Self>
    where
        T: HasWindowHandle + HasDisplayHandle,
    {
        let ext = ash::khr::surface::Instance::new(entry, instance);
        let handle = unsafe {
            ash_window::create_surface(
                entry,
                instance,
                window.display_handle()?.as_raw(),
                window.window_handle()?.as_raw(),
                None,
            )?
        };

        #[cfg(feature = "log-lifetimes")]
        trace!("Creating VkSurface {:p}", handle);

        Ok(Self {
            handle,
            capabilities: None,
            formats: None,
            present_modes: None,
            ext,
        })
    }

    /// Determine the [`vk::SurfaceCapabilitiesKHR`] and [`vk::SurfaceFormatKHR`] and [`vk::PresentModeKHR`]
    pub fn query_details(&mut self, physical_device: vk::PhysicalDevice) -> Result<()> {
        self.capabilities = Some(unsafe {
            self.ext
                .get_physical_device_surface_capabilities(physical_device, self.handle)?
        });
        self.present_modes = Some(unsafe {
            self.ext
                .get_physical_device_surface_present_modes(physical_device, self.handle)?
        });
        self.formats = Some(unsafe {
            self.ext
                .get_physical_device_surface_formats(physical_device, self.handle)?
        });
        Ok(())
    }

    /// Get the [`vk::SurfaceCapabilitiesKHR`] of the [`Surface`]
    pub fn get_capabilities(&self) -> Option<vk::SurfaceCapabilitiesKHR> {
        self.capabilities
    }

    /// Get the [`vk::SurfaceFormatKHR`] of a [`Surface`]
    pub fn get_formats(&self) -> &Option<Vec<vk::SurfaceFormatKHR>> {
        &self.formats
    }

    /// Get the [`vk::PresentModeKHR`] of a [`Surface`]
    pub fn get_present_modes(&self) -> &Option<Vec<vk::PresentModeKHR>> {
        &self.present_modes
    }

    /// Get a reference to the underlying [SurfaceKHR](vk::SurfaceKHR)
    pub fn get_handle(&self) -> &vk::SurfaceKHR {
        &self.handle
    }

    /// Get a copy over the underlying [SurfaceKHR}(vk::SurfaceKHR)
    pub fn handle(&self) -> vk::SurfaceKHR {
        self.handle
    }

    /// Gets all extensions available
    pub fn get_extension(&self) -> &ash::khr::surface::Instance {
        &self.ext
    }
}

impl Destructible for Surface {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        trace!("Destroying VkSurface {:p}", self.handle);

        unsafe {
            self.ext.destroy_surface(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl Drop for Surface {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl Surface {}
