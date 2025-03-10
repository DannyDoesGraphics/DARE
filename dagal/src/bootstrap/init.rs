use crate::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl};
use crate::bootstrap::app_info::AppSettings;
use crate::traits::AsRaw;
use ash::vk;
use gpu_allocator::vulkan::AllocatorCreateDesc;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CString};
use std::marker::PhantomData;
use std::ptr;

pub struct WindowlessContext<W: crate::wsi::DagalWindow> {
    _phantom: PhantomData<W>,
}

pub struct WindowedContext<W: crate::wsi::DagalWindow> {
    _phantom: PhantomData<W>,
}

pub trait ContextInit<W: crate::wsi::DagalWindow> {
    type Output<A: Allocator>;

    /// Initialize the context with the default allocator
    fn init(settings: AppSettings<W>) -> anyhow::Result<Self::Output<GPUAllocatorImpl>>;

    /// Initializes the context with a custom allocator
    fn init_with_allocator<
        A: Allocator,
        F: FnOnce(
            &crate::core::Instance,
            &crate::device::PhysicalDevice,
            &crate::device::LogicalDevice,
        ) -> anyhow::Result<A>,
    >(
        settings: AppSettings<W>,
        make_alloc: F,
    ) -> anyhow::Result<Self::Output<A>>;
}

impl<W: crate::wsi::DagalWindow> ContextInit<W> for WindowlessContext<W> {
    type Output<A: Allocator> = (
        crate::core::Instance,
        crate::device::PhysicalDevice,
        crate::device::LogicalDevice,
        ArcAllocator<A>,
        crate::device::execution_manager::ExecutionManager,
    );

    fn init(settings: AppSettings<W>) -> anyhow::Result<Self::Output<GPUAllocatorImpl>> {
        todo!()
    }

    fn init_with_allocator<
        A: Allocator,
        F: FnOnce(
            &crate::core::Instance,
            &crate::device::PhysicalDevice,
            &crate::device::LogicalDevice,
        ) -> anyhow::Result<A>,
    >(
        settings: AppSettings<W>,
        make_alloc: F,
    ) -> anyhow::Result<Self::Output<A>> {
        todo!()
    }
}

impl<W: crate::wsi::DagalWindow> ContextInit<W> for WindowedContext<W> {
    type Output<A: Allocator> = (
        crate::core::Instance,
        crate::device::PhysicalDevice,
        Option<crate::wsi::Surface>,
        crate::device::LogicalDevice,
        ArcAllocator<A>,
        crate::device::execution_manager::ExecutionManager,
    );

    fn init(settings: AppSettings<W>) -> anyhow::Result<Self::Output<GPUAllocatorImpl>> {
        let application_name: CString = CString::new(settings.name.clone())?;
        let engine_name: CString = CString::new(settings.engine_name.clone())?;
        let application_info = unsafe {
            vk::ApplicationInfo {
                s_type: vk::StructureType::APPLICATION_INFO,
                p_next: ptr::null(),
                p_application_name: application_name.as_ptr(),
                application_version: settings.version,
                p_engine_name: engine_name.as_ptr(),
                engine_version: settings.engine_version,
                api_version: vk::make_api_version(
                    settings.api_version.0,
                    settings.api_version.1,
                    settings.api_version.2,
                    settings.api_version.3,
                ),
                _marker: Default::default(),
            }
        };
        let mut layers: Vec<CString> = Vec::new();
        if settings.enable_validation {
            layers.push(CString::new("VK_LAYER_KHRONOS_validation")?);
        }
        let display_handle = settings.window.map(|r| r.raw_display_handle());
        let window_handle = settings.window.map(|r| r.raw_window_handle());
        let mut extensions: Vec<CString> = Vec::new();
        if let Some(display_handle) = display_handle.clone() {
            let display_handle = display_handle?;
            for ext in ash_window::enumerate_required_extensions(display_handle)? {
                extensions.push(crate::util::wrap_c_str(*ext));
            }
        }
        if settings.debug_utils {
            extensions.push(
                CString::new(
                    ash::ext::debug_utils::NAME
                        .to_string_lossy()
                        .to_string()
                        .as_str(),
                )
                .unwrap(),
            )
        }

        let layers_ptr: Vec<*const c_char> = layers.iter().map(|s| s.as_ptr()).collect();
        let extensions_ptr: Vec<*const c_char> = extensions.iter().map(|s| s.as_ptr()).collect();
        let instance = unsafe {
            crate::core::Instance::new(vk::InstanceCreateInfo {
                s_type: vk::StructureType::INSTANCE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::InstanceCreateFlags::empty(),
                p_application_info: &application_info,
                enabled_layer_count: layers_ptr.len() as u32,
                pp_enabled_layer_names: layers_ptr.as_ptr(),
                enabled_extension_count: extensions_ptr.len() as u32,
                pp_enabled_extension_names: extensions_ptr.as_ptr(),
                _marker: Default::default(),
            })?
        };
        let surface: Option<crate::wsi::Surface> =
            if let (Some(display_handle), Some(window_handle)) = (display_handle, window_handle) {
                crate::wsi::Surface::new_with_handles(
                    instance.get_entry(),
                    instance.get_instance(),
                    display_handle?,
                    window_handle?,
                )
                .map_or_else(
                    |err| {
                        tracing::error!("Failed to construct surface: {:?}", err);
                        None
                    },
                    Some,
                )
            } else {
                None
            };

        let mut features_3 = settings.gpu_requirements.features_3;
        let mut features_2 = settings.gpu_requirements.features_2;
        features_2.p_next = &mut features_3 as *mut _ as *mut c_void;
        let mut features_1 = settings.gpu_requirements.features_1;
        features_1.p_next = &mut features_2 as *mut _ as *mut c_void;
        let features2 = vk::PhysicalDeviceFeatures2 {
            s_type: vk::StructureType::PHYSICAL_DEVICE_FEATURES_2,
            p_next: &mut features_1 as *mut _ as *mut c_void,
            features: settings.gpu_requirements.features,
            _marker: Default::default(),
        };
        let debug_utils = settings.debug_utils;
        let physical_device =
            crate::device::PhysicalDevice::select(&instance, surface.as_ref(), settings)?;
        let queue_priorities: Vec<f32> = vec![1.0f32; physical_device.get_active_queues().len()];
        let active_queues: Vec<vk::DeviceQueueCreateInfo> = physical_device
            .get_active_queues()
            .iter()
            .map(|queue| vk::DeviceQueueCreateInfo {
                s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::DeviceQueueCreateFlags::empty(),
                queue_family_index: queue.family_index,
                queue_count: 1,
                p_queue_priorities: queue_priorities.as_ptr(),
                _marker: Default::default(),
            })
            .collect();

        let enable_extensions: Vec<CString> = physical_device
            .get_extensions()
            .iter()
            .map(|s| CString::new(s.clone()).unwrap())
            .collect();
        let p_enable_extension: Vec<*const c_char> =
            enable_extensions.iter().map(|s| s.as_ptr()).collect();

        // ensure our queue families are unique (merge same queue families together)
        let queue_cis: Vec<vk::DeviceQueueCreateInfo> = {
            let mut family_hashmap: HashMap<u32, vk::DeviceQueueCreateInfo> = HashMap::new();
            for mut queue in active_queues {
                let count = queue.queue_count;
                family_hashmap
                    .entry(queue.queue_family_index)
                    .or_insert_with(|| {
                        queue.queue_count = 0;
                        queue
                    })
                    .queue_count += count;
            }
            family_hashmap
        }
        .into_values()
        .collect::<Vec<vk::DeviceQueueCreateInfo>>();
        let logical_device =
            crate::device::LogicalDevice::new(crate::device::LogicalDeviceCreateInfo {
                instance: instance.get_instance(),
                physical_device: physical_device.clone(),
                device_ci: vk::DeviceCreateInfo {
                    s_type: vk::StructureType::DEVICE_CREATE_INFO,
                    p_next: &features2 as *const _ as *const c_void,
                    flags: vk::DeviceCreateFlags::empty(),
                    queue_create_info_count: queue_cis.len() as u32,
                    p_queue_create_infos: queue_cis.as_ptr(),
                    enabled_layer_count: 0,
                    pp_enabled_layer_names: ptr::null(),
                    enabled_extension_count: p_enable_extension.len() as u32,
                    pp_enabled_extension_names: p_enable_extension.as_ptr(),
                    p_enabled_features: ptr::null(),
                    _marker: Default::default(),
                },
                debug_utils,
            })?;

        // Now we create an execution manager using all queues
        let execution_manager: crate::device::ExecutionManager =
            crate::device::ExecutionManager::from_device(logical_device.clone(), &physical_device);

        // Make an allocator
        let allocator = ArcAllocator::new(GPUAllocatorImpl::new(
            unsafe {
                AllocatorCreateDesc {
                    instance: instance.get_instance().clone(),
                    device: logical_device.get_handle().clone(),
                    physical_device: *physical_device.as_raw(),
                    debug_settings: Default::default(),
                    buffer_device_address: true,
                    allocation_sizes: Default::default(),
                }
            },
            logical_device.clone(),
        )?);

        Ok((
            instance,
            physical_device,
            surface,
            logical_device,
            allocator,
            execution_manager,
        ))
    }

    fn init_with_allocator<
        A: Allocator,
        F: FnOnce(
            &crate::core::Instance,
            &crate::device::PhysicalDevice,
            &crate::device::LogicalDevice,
        ) -> anyhow::Result<A>,
    >(
        settings: AppSettings<W>,
        make_alloc: F,
    ) -> anyhow::Result<Self::Output<A>> {
        todo!()
    }
}
