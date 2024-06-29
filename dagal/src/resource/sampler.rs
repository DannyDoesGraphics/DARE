use anyhow::Result;
use ash::vk;
use ash::vk::Handle;

use crate::resource::traits::{Nameable, Resource};
use crate::traits::Destructible;

#[derive(Debug)]
pub struct Sampler {
    handle: vk::Sampler,
    device: crate::device::LogicalDevice,
}

impl Destructible for Sampler {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        trace!("Destroying VkSampler {:p}", self.handle);
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
    ///     dagal::resource::SamplerCreateInfo::FromCreateInfo {
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
    FromCreateInfo {
        device: crate::device::LogicalDevice,
        create_info: vk::SamplerCreateInfo<'a>,
        name: Option<&'a str>,
    },
}

impl<'a> Resource<'a> for Sampler {
    type CreateInfo = SamplerCreateInfo<'a>;
    type HandleType = vk::Sampler;

    fn new(create_info: Self::CreateInfo) -> Result<Self>
    where
        Self: Sized,
    {
        match create_info {
            SamplerCreateInfo::FromCreateInfo {
                device,
                create_info,
                name,
            } => {
                let handle = unsafe { device.get_handle().create_sampler(&create_info, None) }?;
                #[cfg(feature = "log-lifetimes")]
                trace!("Creating VkSampler {:p}", handle);

                let mut handle = Self { handle, device };
                crate::resource::traits::update_name(&mut handle, name).unwrap_or(Ok(()))?;

                Ok(handle)
            }
        }
    }

    fn get_handle(&self) -> &Self::HandleType {
        &self.handle
    }

    fn handle(&self) -> Self::HandleType {
        self.handle
    }

    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }
}

impl Nameable for Sampler {
    const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::SAMPLER;
    fn set_name(
        &mut self,
        debug_utils: &ash::ext::debug_utils::Device,
        name: &str,
    ) -> anyhow::Result<()> {
        crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)?;
        Ok(())
    }
}
