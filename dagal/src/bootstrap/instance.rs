use anyhow::Result;
use ash::vk;
use std::collections::HashSet;
use std::ffi::{c_char, CString};
use std::ptr;

/// Quickly builds an Instance
pub struct InstanceBuilder<'a> {
    handle: vk::InstanceCreateInfo<'a>,
    /// All instance level extensions used
    extensions: HashSet<CString>,
    /// All layers used
    layers: HashSet<CString>,
    /// Whether to enable validation
    validate: bool,
    /// Set app information
    application_info: vk::ApplicationInfo<'a>,

    /// Vulkan version used
    ///
    /// In the form of (major, minor, patch)
    vulkan_version: (u32, u32, u32),
}

impl<'a> Default for InstanceBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> InstanceBuilder<'a> {
    pub fn new() -> Self {
        Self {
            handle: Default::default(),
            extensions: HashSet::new(),
            layers: HashSet::new(),
            validate: false,
            application_info: Default::default(),
            vulkan_version: (1, 0, 0),
        }
    }

    /// Enables validation
    pub fn set_validation(mut self, validate: bool) -> Self {
        self.validate = validate;
        self
    }

    /// Set vulkan version
    pub fn set_vulkan_version(mut self, version: (u32, u32, u32)) -> Self {
        assert!(
            version.0 <= 0b1111111,
            "Major version must be a 7-bit integer"
        );
        assert!(
            version.1 <= 0b1111111111,
            "Minor version must be a 10-bit integer"
        );
        assert!(
            version.2 <= 0b111111111111,
            "Patch version must be a 12-bit integer"
        );
        self.vulkan_version = version;
        self
    }

    /// Adds an extension
    pub fn add_extension(mut self, name: *const c_char) -> Self {
        self.extensions.insert(crate::util::wrap_c_str(name));
        self
    }

    /// Adds a layer
    pub fn add_layer(mut self, name: *const c_char) -> Self {
        self.layers.insert(crate::util::wrap_c_str(name));
        self
    }

    /// Set app name
    ///
    /// Input in the form of (major, minor, patch)
    pub fn set_application_info(mut self, app_info: vk::ApplicationInfo<'a>) -> Self {
        self.application_info = app_info;
        self
    }

    pub fn build(mut self) -> Result<crate::core::Instance> {
        // link
        let mut instance_ci = self.handle;
        instance_ci.s_type = vk::StructureType::INSTANCE_CREATE_INFO;
        instance_ci.p_next = ptr::null();
        let mut app_information = self.application_info;
        app_information.s_type = vk::StructureType::APPLICATION_INFO;
        app_information.p_next = ptr::null();
        instance_ci.p_application_info = &app_information;

        if self.validate {
            self.layers
                .insert(CString::new("VK_LAYER_KHRONOS_validation")?);
            self.extensions.insert(crate::util::wrap_c_str(
                ash::ext::debug_utils::NAME.as_ptr(),
            ));
        }

        instance_ci.enabled_extension_count = self.extensions.len() as u32;
        let ext_cstring: Vec<CString> = self
            .extensions
            .into_iter()
            .map(|name| CString::new(name).unwrap())
            .collect();
        let ext_cptrs: Vec<*const c_char> = ext_cstring.iter().map(|name| name.as_ptr()).collect();
        instance_ci.pp_enabled_extension_names = ext_cptrs.as_ptr();

        instance_ci.enabled_layer_count = self.layers.len() as u32;
        let layer_cstring: Vec<CString> = self
            .layers
            .into_iter()
            .map(|name| CString::new(name).unwrap())
            .collect();
        let layer_cptrs: Vec<*const c_char> =
            layer_cstring.iter().map(|name| name.as_ptr()).collect();
        instance_ci.pp_enabled_layer_names = layer_cptrs.as_ptr();

        app_information.api_version = vk::make_api_version(
            0,
            self.vulkan_version.0,
            self.vulkan_version.1,
            self.vulkan_version.2,
        );

        crate::core::Instance::new(instance_ci)
    }
}
