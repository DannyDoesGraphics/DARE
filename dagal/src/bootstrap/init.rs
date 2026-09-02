use crate::allocators::{Allocator, GPUAllocatorImpl};
use crate::bootstrap::app_info::AppSettings;
use crate::traits::AsRaw;
use ash::vk;
use gpu_allocator::vulkan::AllocatorCreateDesc;
use std::collections::HashMap;
use std::ffi::{CString, c_char, c_void};

pub struct Context {}

pub trait ContextInit {
    type Output<A: Allocator>;

    /// Initialize the context with the default allocator
    fn init(settings: AppSettings) -> anyhow::Result<Self::Output<GPUAllocatorImpl>>;

    /// Initializes the context with a custom allocator
    fn init_with_allocator<
        A: Allocator,
        F: FnOnce(
            &crate::core::Instance,
            &crate::device::PhysicalDevice,
            &crate::device::LogicalDevice,
        ) -> anyhow::Result<A>,
    >(
        settings: AppSettings,
        make_alloc: F,
    ) -> anyhow::Result<Self::Output<A>>;
}

impl ContextInit for Context {
    type Output<A: Allocator> = (
        crate::core::Instance,
        crate::device::PhysicalDevice,
        Option<crate::wsi::Surface>,
        crate::device::LogicalDevice,
        A,
    );

    fn init(settings: AppSettings) -> anyhow::Result<Self::Output<GPUAllocatorImpl>> {
        let application_name: CString = CString::new(settings.name.clone())?;
        let engine_name: CString = CString::new(settings.engine_name.clone())?;
        let application_info = vk::ApplicationInfo::default()
            .application_name(application_name.as_c_str())
            .application_version(settings.version)
            .engine_name(engine_name.as_c_str())
            .engine_version(settings.engine_version)
            .api_version(vk::make_api_version(
                settings.api_version.0,
                settings.api_version.1,
                settings.api_version.2,
                settings.api_version.3,
            ));
        let mut layers: Vec<CString> = Vec::new();
        if settings.enable_validation {
            layers.push(CString::new("VK_LAYER_KHRONOS_validation")?);
        }
        let mut extensions: Vec<CString> = Vec::new();
        if let Some(display_handle) = settings.raw_display_handle.as_ref() {
            for ext in ash_window::enumerate_required_extensions(*display_handle)? {
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
        let mut instance = crate::core::Instance::new(
            vk::InstanceCreateInfo::default()
                .application_info(&application_info)
                .enabled_layer_names(&layers_ptr)
                .enabled_extension_names(&extensions_ptr),
        )?;
        if settings.debug_utils {
            instance.attach_debug_messenger()?;
        }
        let surface: Option<crate::wsi::Surface> =
            if let (Some(display_handle), Some(window_handle)) =
                (settings.raw_display_handle, settings.raw_window_handle)
            {
                crate::wsi::Surface::new_with_handles(
                    instance.get_entry(),
                    instance.get_instance(),
                    display_handle,
                    window_handle,
                )
                .map_or_else(
                    |err| {
                        log::error!("Failed to construct surface: {:?}", err);
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
        let mut features2 = vk::PhysicalDeviceFeatures2::default();
        features2.p_next = &mut features_1 as *mut _ as *mut c_void;
        features2.features = settings.gpu_requirements.features;
        let debug_utils = settings.debug_utils;
        let physical_device =
            crate::device::PhysicalDevice::select(&instance, surface.as_ref(), settings)?;
        let queue_priorities: Vec<f32> = vec![1.0f32; physical_device.get_active_queues().len()];
        let active_queues: Vec<vk::DeviceQueueCreateInfo> = physical_device
            .get_active_queues()
            .iter()
            .map(|queue| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(queue.family_index)
                    .queue_priorities(&queue_priorities[..1])
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
        #[allow(deprecated)]
        let logical_device =
            crate::device::LogicalDevice::new(crate::device::LogicalDeviceCreateInfo {
                instance: instance.get_instance(),
                physical_device: physical_device.clone(),
                device_ci: {
                    let mut device_ci = vk::DeviceCreateInfo::default()
                        .queue_create_infos(&queue_cis)
                        .enabled_extension_names(&p_enable_extension);
                    device_ci.p_next = &features2 as *const _ as *const c_void;
                    device_ci
                },
                debug_utils,
            })?;

        // Make an allocator
        let allocator = GPUAllocatorImpl::new(
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
        )?;

        Ok((
            instance,
            physical_device,
            surface,
            logical_device,
            allocator,
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
        _settings: AppSettings,
        _make_alloc: F,
    ) -> anyhow::Result<Self::Output<A>> {
        todo!()
    }
}
