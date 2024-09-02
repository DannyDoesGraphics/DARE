use ash::vk;
use ash::vk::Handle;

use crate::resource::traits::{Nameable, Resource};
use crate::traits::AsRaw;

#[derive(Debug)]
pub struct PipelineLayout {
    handle: vk::PipelineLayout,
    device: crate::device::LogicalDevice,
}

pub enum PipelineLayoutCreateInfo<'a> {
    FromVk {
        layout: vk::PipelineLayout,
        device: crate::device::LogicalDevice,
    },
    CreateInfo {
        create_info: vk::PipelineLayoutCreateInfo<'a>,
        name: Option<&'a str>,
        device: crate::device::LogicalDevice,
    },
}

impl Drop for PipelineLayout {
    fn drop(&mut self) {
        unsafe {
            self.device
                .get_handle()
                .destroy_pipeline_layout(self.handle, None);
        }
    }
}

impl<'a> Resource<'a> for PipelineLayout {
    type CreateInfo = PipelineLayoutCreateInfo<'a>;

    fn new(create_info: Self::CreateInfo) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let handle = match create_info {
            PipelineLayoutCreateInfo::CreateInfo {
                create_info,
                name,
                device,
            } => {
                let handle = unsafe {
                    device
                        .get_handle()
                        .create_pipeline_layout(&create_info, None)
                }
                .unwrap();
                let mut handle = Self { handle, device };
                if let Some(name) = name {
                    if let Some(debug_utils) = handle.device.clone().get_debug_utils() {
                        handle.set_name(debug_utils, name)?;
                    }
                }
                handle
            }
            PipelineLayoutCreateInfo::FromVk {
                layout: pipeline,
                device,
            } => Self {
                handle: pipeline,
                device,
            },
        };

        Ok(handle)
    }

    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }
}

impl AsRaw for PipelineLayout {
    type RawType = vk::PipelineLayout;

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

impl Nameable for PipelineLayout {
    const OBJECT_TYPE: vk::ObjectType = vk::ObjectType::PIPELINE_LAYOUT;

    fn set_name(
        &mut self,
        debug_utils: &ash::ext::debug_utils::Device,
        name: &str,
    ) -> anyhow::Result<()> {
        crate::resource::traits::name_nameable::<Self>(debug_utils, self.handle.as_raw(), name)
    }
}
