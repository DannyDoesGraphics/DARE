use std::marker::PhantomData;
use std::ptr;
use std::time::Duration;

use crate::bootstrap::app_info::{AppSettings, Expected, GPURequirements, QueueRequest};
use crate::bootstrap::init::{Context, ContextInit};
use ash;
use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

pub struct TestHarness {
    immediate: std::sync::Mutex<crate::command::ImmediateSubmit>,
    queue_info: crate::device::QueueInfo,
    allocator: crate::allocators::GPUAllocatorImpl,
    surface: Option<crate::wsi::SurfaceQueried>,
    device: crate::device::LogicalDevice,
    physical_device: crate::device::PhysicalDevice,
    instance: crate::core::Instance,
    window: Option<winit::window::Window>,
    event_loop: Option<winit::event_loop::EventLoop<()>>,
}

unsafe impl Send for TestHarness {}
unsafe impl Sync for TestHarness {}

impl Drop for TestHarness {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.get_handle().device_wait_idle();
        }
    }
}

impl TestHarness {
    pub fn headless() -> TestHarnessBuilder {
        TestHarnessBuilder::new(false)
    }

    pub fn windowed() -> TestHarnessBuilder {
        TestHarnessBuilder::new(true)
    }

    fn from_parts(
        settings: AppSettings,
        window: Option<winit::window::Window>,
        event_loop: Option<winit::event_loop::EventLoop<()>>,
    ) -> anyhow::Result<Self> {
        let (instance, physical_device, surface, device, allocator) = Context::init(settings)?;
        let surface = match surface {
            Some(surface) => Some(surface.query_details(physical_device.handle())?),
            None => None,
        };
        let queue_info = *physical_device.get_active_queues().last().unwrap();
        let queue = unsafe {
            device.get_queue(
                &vk::DeviceQueueInfo2 {
                    s_type: vk::StructureType::DEVICE_QUEUE_INFO_2,
                    p_next: ptr::null(),
                    flags: vk::DeviceQueueCreateFlags::empty(),
                    queue_family_index: queue_info.family_index,
                    queue_index: queue_info.index,
                    _marker: PhantomData,
                },
                queue_info.queue_flags,
                queue_info.strict,
                queue_info.can_present,
            )
        };
        let immediate = crate::command::ImmediateSubmit::new(device.clone(), queue)?;
        Ok(Self {
            immediate: std::sync::Mutex::new(immediate),
            queue_info,
            allocator,
            surface,
            device,
            physical_device,
            instance,
            window,
            event_loop,
        })
    }

    pub fn instance(&self) -> &crate::core::Instance {
        &self.instance
    }

    pub fn physical_device(&self) -> &crate::device::PhysicalDevice {
        &self.physical_device
    }

    pub fn device(&self) -> crate::device::LogicalDevice {
        self.device.clone()
    }

    pub fn allocator(&self) -> crate::allocators::GPUAllocatorImpl {
        self.allocator.clone()
    }

    pub fn queue(&self, index: usize) -> crate::device::Queue {
        let queue_info = self.physical_device.get_active_queues()[index];
        unsafe {
            self.device.get_queue(
                &vk::DeviceQueueInfo2 {
                    s_type: vk::StructureType::DEVICE_QUEUE_INFO_2,
                    p_next: ptr::null(),
                    flags: vk::DeviceQueueCreateFlags::empty(),
                    queue_family_index: queue_info.family_index,
                    queue_index: queue_info.index,
                    _marker: PhantomData,
                },
                queue_info.queue_flags,
                queue_info.strict,
                queue_info.can_present,
            )
        }
    }

    pub fn queue_info(&self) -> crate::device::QueueInfo {
        self.queue_info
    }

    pub fn window(&self) -> &winit::window::Window {
        self.window
            .as_ref()
            .expect("window() called on a headless TestHarness; use TestHarness::windowed()")
    }

    pub fn surface(&self) -> &crate::wsi::SurfaceQueried {
        self.surface
            .as_ref()
            .expect("surface() called on a headless TestHarness; use TestHarness::windowed()")
    }

    pub fn immediate_submit<F, R>(&self, f: F) -> crate::Result<R>
    where
        F: FnOnce(&Self, &crate::command::CommandBufferRecording) -> R,
    {
        let mut immediate = self.immediate.lock()?;
        immediate.submit(|recording| f(self, recording))
    }
}

pub struct TestHarnessBuilder {
    windowed: bool,
    api_version: (u32, u32, u32),
    extensions: Vec<String>,
    queues: Vec<QueueRequest>,
    event_hook: Option<Box<dyn FnMut(&winit::event::WindowEvent)>>,
}

impl TestHarnessBuilder {
    fn new(windowed: bool) -> Self {
        Self {
            windowed,
            api_version: (1, 3, 0),
            extensions: Vec::new(),
            queues: vec![QueueRequest {
                strict: false,
                queue_type: vec![Expected::Required(
                    vk::QueueFlags::GRAPHICS | vk::QueueFlags::TRANSFER | vk::QueueFlags::COMPUTE,
                )]
                .into(),
                count: Expected::Required(2),
            }],
            event_hook: None,
        }
    }

    pub fn expect_extension(mut self, extensions: &[&str]) -> Self {
        self.extensions
            .extend(extensions.iter().map(|name| name.to_string()));
        self
    }

    pub fn expect_vk_version(mut self, major: u32, minor: u32, patch: u32) -> Self {
        self.api_version = (major, minor, patch);
        self
    }

    pub fn expect_queues(mut self, queues: &[QueueRequest]) -> Self {
        self.queues = queues.to_vec();
        self
    }

    pub fn event_loop<F>(mut self, hook: F) -> Self
    where
        F: FnMut(&winit::event::WindowEvent) + 'static,
    {
        self.event_hook = Some(Box::new(hook));
        self
    }

    pub fn build(mut self) -> anyhow::Result<TestHarness> {
        let (major, minor, patch) = self.api_version;
        let (window, event_loop) = if self.windowed {
            let (window, event_loop) = create_window(self.event_hook.take())?;
            (Some(window), Some(event_loop))
        } else {
            (None, None)
        };
        let settings = self.settings(window.as_ref());
        let harness = TestHarness::from_parts(settings, window, event_loop)?;

        let supported = harness.physical_device.get_properties().api_version;
        let (have_major, have_minor) = (
            vk::api_version_major(supported),
            vk::api_version_minor(supported),
        );
        if have_major < major || (have_major == major && have_minor < minor) {
            anyhow::bail!(
                "selected device supports Vulkan {have_major}.{have_minor} but {major}.{minor}.{patch} was required"
            );
        }
        Ok(harness)
    }

    fn settings(&self, window: Option<&winit::window::Window>) -> AppSettings {
        let mut device_extensions: Vec<Expected<String>> = self
            .extensions
            .iter()
            .map(|name| Expected::Required(name.clone()))
            .collect();
        let mut raw_display_handle = None;
        let mut raw_window_handle = None;
        let mut present_mode = None;
        if let Some(window) = window {
            raw_display_handle = Some(window.display_handle().unwrap().as_raw());
            raw_window_handle = Some(window.window_handle().unwrap().as_raw());
            present_mode = Some(Expected::Preferred(vk::PresentModeKHR::FIFO));
            let swapchain = ash::khr::swapchain::NAME.to_string_lossy().to_string();
            if !self.extensions.iter().any(|name| *name == swapchain) {
                device_extensions.push(Expected::Required(swapchain));
            }
        }
        AppSettings {
            name: "dagal-tests".to_string(),
            version: 0,
            engine_name: "dagal-tests".to_string(),
            engine_version: 0,
            api_version: (
                0,
                self.api_version.0,
                self.api_version.1,
                self.api_version.2,
            ),
            enable_validation: true,
            debug_utils: true,
            raw_display_handle,
            raw_window_handle,
            surface_format: None,
            present_mode,
            gpu_requirements: GPURequirements {
                dedicated: Expected::Preferred(true),
                features: vk::PhysicalDeviceFeatures {
                    shader_int64: vk::TRUE,
                    ..Default::default()
                },
                features_1: vk::PhysicalDeviceVulkan11Features {
                    s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_FEATURES,
                    variable_pointers: vk::TRUE,
                    variable_pointers_storage_buffer: vk::TRUE,
                    shader_draw_parameters: vk::TRUE,
                    ..Default::default()
                },
                features_2: vk::PhysicalDeviceVulkan12Features {
                    s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_2_FEATURES,
                    buffer_device_address: vk::TRUE,
                    descriptor_indexing: vk::TRUE,
                    descriptor_binding_partially_bound: vk::TRUE,
                    runtime_descriptor_array: vk::TRUE,
                    scalar_block_layout: vk::TRUE,
                    timeline_semaphore: vk::TRUE,
                    ..Default::default()
                },
                features_3: vk::PhysicalDeviceVulkan13Features {
                    s_type: vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_3_FEATURES,
                    dynamic_rendering: vk::TRUE,
                    synchronization2: vk::TRUE,
                    ..Default::default()
                },
                device_extensions,
                queues: self.queues.clone(),
            },
        }
    }
}

fn create_window(
    hook: Option<Box<dyn FnMut(&winit::event::WindowEvent)>>,
) -> anyhow::Result<(winit::window::Window, winit::event_loop::EventLoop<()>)> {
    use winit::platform::pump_events::EventLoopExtPumpEvents;

    let mut builder = winit::event_loop::EventLoop::builder();
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use winit::platform::wayland::EventLoopBuilderExtWayland;
        use winit::platform::x11::EventLoopBuilderExtX11;
        EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    }
    let mut event_loop = builder.build()?;

    struct Creator {
        window: Option<winit::window::Window>,
        hook: Option<Box<dyn FnMut(&winit::event::WindowEvent)>>,
    }
    impl winit::application::ApplicationHandler for Creator {
        fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
            if self.window.is_none() {
                self.window = Some(
                    event_loop
                        .create_window(
                            winit::window::WindowAttributes::default()
                                .with_title("dagal test window"),
                        )
                        .unwrap(),
                );
            }
        }

        fn window_event(
            &mut self,
            _event_loop: &winit::event_loop::ActiveEventLoop,
            _window_id: winit::window::WindowId,
            event: winit::event::WindowEvent,
        ) {
            if let Some(hook) = self.hook.as_mut() {
                hook(&event);
            }
        }
    }

    let mut creator = Creator { window: None, hook };
    let mut spins = 0;
    while creator.window.is_none() && spins < 2048 {
        event_loop.pump_app_events(Some(Duration::from_millis(1)), &mut creator);
        spins += 1;
    }
    let window = creator
        .window
        .ok_or_else(|| anyhow::anyhow!("failed to create a test window"))?;
    Ok((window, event_loop))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_builds() {
        let ctx = TestHarness::headless().build().unwrap();
        drop(ctx);
    }

    #[test]
    fn missing_extension_fails() {
        let result = TestHarness::headless()
            .expect_extension(&["VK_EXT_this_extension_does_not_exist"])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn unsupported_version_fails() {
        let result = TestHarness::headless().expect_vk_version(9, 9, 0).build();
        assert!(result.is_err());
    }
}
