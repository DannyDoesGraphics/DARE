use anyhow::Result;
use ash;
use ash::vk;
use derivative::Derivative;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::traits::{AsRaw, Destructible};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SurfaceQueried {
    inner: Surface,
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
}

impl SurfaceQueried {
    /// Get a reference to the underlying [SurfaceKHR](vk::SurfaceKHR)
    pub fn get_handle(&self) -> &vk::SurfaceKHR {
        &self.inner.handle
    }

    /// Get a copy over the underlying [SurfaceKHR}(vk::SurfaceKHR)
    pub fn handle(&self) -> vk::SurfaceKHR {
        self.inner.handle
    }

    /// Gets all extensions available
    pub fn get_extension(&self) -> &ash::khr::surface::Instance {
        &self.inner.ext
    }

    /// Get the [`vk::SurfaceCapabilitiesKHR`] of the [`Surface`]
    pub fn get_capabilities(&self) -> vk::SurfaceCapabilitiesKHR {
        self.capabilities
    }

    /// Get the [`vk::SurfaceFormatKHR`] of a [`Surface`]
    pub fn get_formats(&self) -> &[vk::SurfaceFormatKHR] {
        self.formats.as_ref()
    }

    /// Get the [`vk::PresentModeKHR`] of a [`Surface`]
    pub fn get_present_modes(&self) -> &[vk::PresentModeKHR] {
        self.present_modes.as_ref()
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Surface {
    handle: vk::SurfaceKHR,
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
    ///     let test_vulkan = dagal::util::tests::create_vulkan_and_device(
    ///         TestSettings::from_rdh(window.display_handle().unwrap().as_raw())
    /// 	);
    ///     // Construct a surface
    ///     let mut surface: dagal::wsi::Surface = dagal::wsi::Surface::new(test_vulkan.instance.get_entry(), test_vulkan.instance.get_instance(), window).unwrap();
    ///     let surface = surface.query_details(test_vulkan.physical_device.as_ref().unwrap().handle()).unwrap();
    ///     assert!(surface.get_capabilities().min_image_count > 0);
    ///     assert!(surface.get_formats().len() > 0);
    ///     assert!(surface.get_present_modes().as_ref().unwrap().len() > 0);
    ///     drop(surface);
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
        tracing::trace!("Creating VkSurface {:p}", handle);

        Ok(Self { handle, ext })
    }

    /// Determine the [`vk::SurfaceCapabilitiesKHR`] and [`vk::SurfaceFormatKHR`] and [`vk::PresentModeKHR`]
    pub fn query_details(self, physical_device: vk::PhysicalDevice) -> Result<SurfaceQueried> {
        let capabilities = unsafe {
            self.ext
                .get_physical_device_surface_capabilities(physical_device, self.handle)?
        };
        let present_modes = unsafe {
            self.ext
                .get_physical_device_surface_present_modes(physical_device, self.handle)?
        };
        let formats = unsafe {
            self.ext
                .get_physical_device_surface_formats(physical_device, self.handle)?
        };
        Ok(SurfaceQueried {
            inner: self,
            capabilities,
            formats,
            present_modes,
        })
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

impl AsRaw for Surface {
    type RawType = vk::SurfaceKHR;

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

impl Destructible for Surface {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Destroying VkSurface {:p}", self.handle);

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
