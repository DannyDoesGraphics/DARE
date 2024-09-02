use std::ffi::{c_char, CString};

use crate::util::{convert_raw_c_ptrs_to_cstring, wrap_c_str};
use crate::wsi::DagalWindow;
use ash;
use ash::vk;
use raw_window_handle::RawDisplayHandle;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;

/// Quick utility stuff for tests
#[derive(Default, Clone)]
pub struct TestSettings {
    /// required instance layers
    pub instance_layers: Vec<CString>,
    /// required instance extensions
    pub instance_extensions: Vec<CString>,
    /// required physical device extensions
    pub physical_device_extensions: Vec<CString>,
    /// required device extensions
    pub logical_device_extensions: Vec<CString>,
}

pub struct TestVulkan {
    pub compute_queue: Option<crate::device::Queue>,
    pub device: Option<crate::device::LogicalDevice>,
    pub debug_messenger: Option<crate::device::DebugMessenger>,
    pub physical_device: Option<crate::device::PhysicalDevice>,
    pub instance: crate::core::Instance,
}

impl TestSettings {
    pub fn from_rdh(rdh: RawDisplayHandle) -> Self {
        Self {
            instance_extensions: convert_raw_c_ptrs_to_cstring(
                ash_window::enumerate_required_extensions(rdh).unwrap(),
            ),
            ..Default::default()
        }
    }

    pub fn add_instance_layer(mut self, layer: *const c_char) -> Self {
        self.instance_layers.push(wrap_c_str(layer));
        self
    }

    pub fn add_physical_device_extension(mut self, ext: *const c_char) -> Self {
        self.physical_device_extensions.push(wrap_c_str(ext));
        self
    }
}

/// Quickly make [`ash::Entry`] and [`ash::Instance`]
///
/// This is used for testing only hence, validation will always be on
pub fn create_vulkan(settings: TestSettings) -> TestVulkan {
    let mut instance = crate::bootstrap::InstanceBuilder::new().set_validation(true);
    for ext in settings.instance_extensions.iter() {
        instance = instance.add_extension(ext.as_ptr());
    }
    for ext in settings.instance_layers.iter() {
        instance = instance.add_layer(ext.as_ptr())
    }
    instance = instance.set_validation(true); // force validation

    let instance = instance.build().unwrap();

    // debug messenger
    let debug_messenger =
        crate::device::DebugMessenger::new(instance.get_entry(), instance.get_instance()).unwrap();
    TestVulkan {
        compute_queue: None,
        device: None,
        debug_messenger: Some(debug_messenger),
        physical_device: None,
        instance,
    }
}

pub fn create_vulkan_and_device(settings: TestSettings) -> TestVulkan {
    let test_vulkan = create_vulkan(settings.clone());
    let compute_queue = crate::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true);
    let mut physical_device_bootstrap = crate::bootstrap::PhysicalDeviceSelector::default()
        .add_required_queue(compute_queue.clone());
    for ext in settings.physical_device_extensions.iter() {
        physical_device_bootstrap = physical_device_bootstrap.add_required_extension(ext.as_ptr());
    }
    let physical_device_bootstrap = physical_device_bootstrap
        .select(test_vulkan.instance.get_instance())
        .unwrap();
    let physical_device = physical_device_bootstrap.handle.clone();
    let mut logical_device =
        crate::bootstrap::LogicalDeviceBuilder::from(physical_device_bootstrap).debug_utils(true);
    for ext in settings.logical_device_extensions.iter() {
        logical_device = logical_device.add_extension(ext.as_ptr());
    }
    let logical_device = logical_device
        .build(test_vulkan.instance.get_instance())
        .unwrap();
    let mut compute_queue = logical_device.acquire_queue(vk::QueueFlags::COMPUTE, Some(true), Some(false), None).unwrap();
    TestVulkan {
        compute_queue: compute_queue.pop(),
        device: Some(logical_device),
        debug_messenger: test_vulkan.debug_messenger,
        physical_device: Some(physical_device),
        instance: test_vulkan.instance,
    }
}

/// Basic test app for unit testing only
#[allow(clippy::type_complexity)]
#[derive(Default)]
pub struct TestApp<T: DagalWindow> {
    window: Option<T>,
    test_function: Option<Box<dyn Fn(&T)>>,
}

impl<T: DagalWindow> TestApp<T> {
    pub fn new() -> Self {
        Self {
            window: None,
            test_function: None,
        }
    }
}

impl TestApp<winit::window::Window> {
    pub fn attach_function<A: Fn(&winit::window::Window) + 'static>(mut self, func: A) -> Self {
        self.test_function = Some(Box::new(func));
        self
    }

    pub fn run(mut self) {
        let event_loop = winit::event_loop::EventLoop::new().unwrap();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

        event_loop.run_app(&mut self).unwrap()
    }
}

#[cfg(feature = "winit")]
impl winit::application::ApplicationHandler for TestApp<winit::window::Window> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window: winit::window::Window = event_loop
            .create_window(
                winit::window::WindowAttributes::default().with_title("Unit test window"),
            )
            .unwrap();
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let window: &winit::window::Window = match self.window.as_ref() {
            None => {
                return;
            }
            Some(window) => window,
        };

        if event == WindowEvent::CloseRequested {
            event_loop.exit();
        };

        self.test_function.as_ref().unwrap()(window);

        event_loop.exit();
    }
}
