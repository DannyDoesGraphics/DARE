use crate::traits::{AsRaw, Destructible};
use ash;
use ash::vk;
use derivative::Derivative;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

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

    pub fn refresh(&mut self, physical_device: vk::PhysicalDevice) -> crate::Result<()> {
        self.capabilities = unsafe {
            self.inner
                .ext
                .get_physical_device_surface_capabilities(physical_device, self.inner.handle)?
        };
        self.present_modes = unsafe {
            self.inner
                .ext
                .get_physical_device_surface_present_modes(physical_device, self.inner.handle)?
        };
        self.formats = unsafe {
            self.inner
                .ext
                .get_physical_device_surface_formats(physical_device, self.inner.handle)?
        };
        Ok(())
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
    /// use dagal::ash::vk;
    /// use dagal::util::tests::TestHarness;
    /// let ctx = TestHarness::windowed().build().unwrap();
    /// // Construct a surface
    /// let surface: dagal::wsi::Surface = dagal::wsi::Surface::new(ctx.instance().get_entry(), ctx.instance().get_instance(), ctx.window()).unwrap();
    /// let surface = surface.query_details(ctx.physical_device().handle()).unwrap();
    /// assert!(surface.get_capabilities().min_image_count > 0);
    /// assert!(surface.get_formats().len() > 0);
    /// assert!(surface.get_present_modes().len() > 0);
    /// drop(surface);
    /// ```
    pub fn new<T>(entry: &ash::Entry, instance: &ash::Instance, window: &T) -> crate::Result<Self>
    where
        T: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle,
    {
        Self::new_with_handles(
            entry,
            instance,
            window
                .display_handle()
                .map_err(|_| crate::DagalError::InvalidWindowHandles)?
                .as_raw(),
            window
                .window_handle()
                .map_err(|_| crate::DagalError::InvalidWindowHandles)?
                .as_raw(),
        )
    }

    /// Similar to [`Self::new`] but you pass in manaully the handles
    pub fn new_with_handles(
        entry: &ash::Entry,
        instance: &ash::Instance,
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
    ) -> crate::Result<Self> {
        let ext = ash::khr::surface::Instance::new(entry, instance);
        let handle = unsafe {
            ash_window::create_surface(entry, instance, display_handle, window_handle, None)?
        };

        #[cfg(feature = "log-lifetimes")]
        log::trace!("Creating VkSurface {:p}", handle);

        Ok(Self { handle, ext })
    }

    /// Determine the [`vk::SurfaceCapabilitiesKHR`] and [`vk::SurfaceFormatKHR`] and [`vk::PresentModeKHR`]
    pub fn query_details(
        self,
        physical_device: vk::PhysicalDevice,
    ) -> crate::Result<SurfaceQueried> {
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
        log::trace!("Destroying VkSurface {:p}", self.handle);

        unsafe {
            self.ext.destroy_surface(self.handle, None);
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl Surface {}
