use crate::resource::traits::{Nameable, Resource};
use crate::traits::{AsRaw, Destructible};
use anyhow::Result;
use ash::vk;
use ash::vk::Handle;
use std::hash::{Hash, Hasher};
use std::ptr;

#[derive(Debug)]
pub struct Sampler {
    handle: vk::Sampler,
    device: crate::device::LogicalDevice,
}
unsafe impl Send for Sampler {}
impl PartialEq for Sampler {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
}
impl Eq for Sampler {}

impl Destructible for Sampler {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Destroying VkSampler {:p}", self.handle);
        unsafe {
            self.device.get_handle().destroy_sampler(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl Drop for Sampler {
    fn drop(&mut self) {
        self.destroy();
    }
}

#[derive(Clone)]
pub enum SamplerCreateInfo<'a> {
    /// Creates a sampler from an existing [`VkSamplerCreateInfo`](vk::SamplerCreateInfo).
    ///
    /// # Examples
    /// ```
    /// use std::ptr;
    /// use ash::vk;
    /// use dagal::resource::traits::Resource;
    /// use dagal::util::tests::TestSettings;
    /// let test_vulkan = dagal::util::tests::create_vulkan_and_device(TestSettings::default());
    /// let sampler = dagal::resource::Sampler::new(
    ///     dagal::resource::SamplerCreateInfo::FromVk {
    ///         device: test_vulkan.device.as_ref().unwrap().clone(),
    /// 		create_info: vk::SamplerCreateInfo {
    ///             s_type: vk::StructureType::SAMPLER_CREATE_INFO,
    /// 			p_next: ptr::null(),
    /// 			..Default::default()
    /// 		},
    /// 		name: None,
    /// }).unwrap();
    /// drop(sampler);
    /// ```
    FromVk {
        device: crate::device::LogicalDevice,
        create_info: vk::SamplerCreateInfo<'a>,
        name: Option<&'a str>,
    },
    FromCreateInfo {
        device: crate::device::LogicalDevice,
        name: Option<&'a str>,
        flags: vk::SamplerCreateFlags,
        mag_filter: vk::Filter,
        min_filter: vk::Filter,
        mipmap_mode: vk::SamplerMipmapMode,
        address_mode_u: vk::SamplerAddressMode,
        address_mode_v: vk::SamplerAddressMode,
        address_mode_w: vk::SamplerAddressMode,
        mip_lod_bias: f32,
        anisotropy_enable: vk::Bool32,
        max_anisotropy: f32,
        compare_enable: vk::Bool32,
        compare_op: vk::CompareOp,
        min_lod: f32,
        max_lod: f32,
        border_color: vk::BorderColor,
        unnormalized_coordinates: vk::Bool32,
    },
}
impl Hash for SamplerCreateInfo<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            SamplerCreateInfo::FromVk {
                device,
                create_info,
                name,
            } => unimplemented!(),
            SamplerCreateInfo::FromCreateInfo { .. } => {}
        }
    }
}
impl PartialEq for SamplerCreateInfo<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                SamplerCreateInfo::FromCreateInfo {
                    device: _,
                    name: a,
                    flags,
                    mag_filter,
                    min_filter,
                    mipmap_mode,
                    address_mode_u,
                    address_mode_v,
                    address_mode_w,
                    mip_lod_bias,
                    anisotropy_enable,
                    max_anisotropy,
                    compare_enable,
                    compare_op,
                    min_lod,
                    max_lod,
                    border_color,
                    unnormalized_coordinates,
                },
                SamplerCreateInfo::FromCreateInfo {
                    device: _,
                    name: b,
                    flags: flags_b,
                    mag_filter: mag_filter_b,
                    min_filter: min_filter_b,
                    mipmap_mode: mipmap_mode_b,
                    address_mode_u: address_mode_u_b,
                    address_mode_v: address_mode_v_b,
                    address_mode_w: address_mode_w_b,
                    mip_lod_bias: mip_lod_bias_b,
                    anisotropy_enable: anisotropy_enable_b,
                    max_anisotropy: max_anisotropy_b,
                    compare_enable: compare_enable_b,
                    compare_op: compare_op_b,
                    min_lod: min_lod_b,
                    max_lod: max_lod_b,
                    border_color: border_color_b,
                    unnormalized_coordinates: unnormalized_coordinates_b,
                },
            ) => {
                a == b
                    && flags == flags_b
                    && mag_filter == mag_filter_b
                    && min_filter == min_filter_b
                    && mipmap_mode == mipmap_mode_b
                    && address_mode_u == address_mode_u_b
                    && address_mode_v == address_mode_v_b
                    && address_mode_w == address_mode_w_b
                    && mip_lod_bias == mip_lod_bias_b
                    && anisotropy_enable == anisotropy_enable_b
                    && max_anisotropy == max_anisotropy_b
                    && compare_enable == compare_enable_b
                    && compare_op == compare_op_b
                    && min_lod == min_lod_b
                    && max_lod == max_lod_b
                    && border_color == border_color_b
                    && unnormalized_coordinates == unnormalized_coordinates_b
            }
            _ => unimplemented!(),
        }
    }
}
impl Eq for SamplerCreateInfo<'_> {}

impl Hash for Sampler {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.handle.hash(state);
    }
}

impl Resource for Sampler {
    type CreateInfo<'a> = SamplerCreateInfo<'a>;

    fn new(create_info: Self::CreateInfo<'_>) -> Result<Self, crate::DagalError>
    where
        Self: Sized,
    {
        match create_info {
            SamplerCreateInfo::FromVk {
                device,
                create_info,
                name,
            } => {
                let handle = unsafe { device.get_handle().create_sampler(&create_info, None) }?;
                #[cfg(feature = "log-lifetimes")]
                tracing::trace!("Creating VkSampler {:p}", handle);

                let mut handle = Self { handle, device };
                crate::resource::traits::update_name(&mut handle, name).unwrap_or(Ok(()))?;

                Ok(handle)
            }
            SamplerCreateInfo::FromCreateInfo {
                device,
                name,
                flags,
                mag_filter,
                min_filter,
                mipmap_mode,
                address_mode_u,
                address_mode_v,
                address_mode_w,
                mip_lod_bias,
                anisotropy_enable,
                max_anisotropy,
                compare_enable,
                compare_op,
                min_lod,
                max_lod,
                border_color,
                unnormalized_coordinates,
            } => {
                let handle = unsafe {
                    device.get_handle().create_sampler(
                        &vk::SamplerCreateInfo {
                            s_type: vk::StructureType::SAMPLER_CREATE_INFO,
                            p_next: ptr::null(),
                            flags,
                            mag_filter,
                            min_filter,
                            mipmap_mode,
                            address_mode_u,
                            address_mode_v,
                            address_mode_w,
                            mip_lod_bias,
                            anisotropy_enable,
                            max_anisotropy,
                            compare_enable,
                            compare_op,
                            min_lod,
                            max_lod,
                            border_color,
                            unnormalized_coordinates,
                            _marker: Default::default(),
                        },
                        None,
                    )
                }?;
                #[cfg(feature = "log-lifetimes")]
                tracing::trace!("Creating VkSampler {:p}", handle);

                let mut handle = Self { handle, device };
                crate::resource::traits::update_name(&mut handle, name).unwrap_or(Ok(()))?;

                Ok(handle)
            }
        }
    }

    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }
}

impl AsRaw for Sampler {
    type RawType = vk::Sampler;

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

impl Nameable for Sampler {
    const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::SAMPLER;
    fn set_name(
        &mut self,
        debug_utils: &ash::ext::debug_utils::Device,
        name: &str,
    ) -> Result<(), crate::DagalError> {
        crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
        Ok(())
    }
}
