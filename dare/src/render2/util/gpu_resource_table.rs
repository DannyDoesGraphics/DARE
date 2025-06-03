use std::ptr;
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

use anyhow::Result;
/// Bevy
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, ArcAllocator};
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use dagal::{descriptor, resource};
use dare_containers::prelude::Container;

#[derive(Debug)]
struct GPUResourceTableInner<A: Allocator> {
    pool: descriptor::DescriptorPool,
    set_layout: descriptor::DescriptorSetLayout,
    descriptor_set: descriptor::DescriptorSet,
    address_buffer: resource::Buffer<A>,
}

#[derive(Debug, Clone, becs::Resource)]
pub struct GPUResourceTable<A: Allocator + 'static> {
    inner: Arc<RwLock<GPUResourceTableInner<A>>>,
    device: dagal::device::LogicalDevice,
}
unsafe impl<A: Allocator + 'static> Send for GPUResourceTable<A> {}
unsafe impl<A: Allocator + 'static> Sync for GPUResourceTable<A> {}

const MAX_IMAGE_RESOURCES: u32 = u16::MAX as u32;
const MAX_BUFFER_RESOURCES: u32 = u16::MAX as u32;
const MAX_SAMPLER_RESOURCES: u32 = u8::MAX as u32;

const BUFFER_BINDING_INDEX: u32 = 3;
const STORAGE_IMAGE_BINDING_INDEX: u32 = 2;
const SAMPLED_IMAGE_BINDING_INDEX: u32 = 1;
const SAMPLER_BINDING_INDEX: u32 = 0;

pub enum ResourceInput<'a, T: Resource> {
    ResourceHandle(T),
    ResourceArc(Arc<T>),
    ResourceWeak(Weak<T>),
    ResourceCIHandle(T::CreateInfo<'a>),
    ResourceCIArc(T::CreateInfo<'a>),
}

impl<A: Allocator> GPUResourceTable<A> {
    pub fn new(
        device: dagal::device::LogicalDevice,
        allocator: &mut ArcAllocator<A>,
    ) -> Result<Self> {
        let pool_sizes = vec![
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::SAMPLER)
                .descriptor_count(MAX_SAMPLER_RESOURCES),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(MAX_IMAGE_RESOURCES),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(MAX_IMAGE_RESOURCES),
            vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1),
        ];

        let pool =
            descriptor::DescriptorPool::new(descriptor::DescriptorPoolCreateInfo::FromPoolSizes {
                sizes: pool_sizes,
                flags: vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND,
                max_sets: 1,
                device: device.clone(),
                name: None,
            })?;
        let set_layout = dagal::descriptor::DescriptorSetLayoutBuilder::default()
            .add_raw_binding(&[
                descriptor::descriptor_set_layout_builder::DescriptorSetLayoutBinding::default()
                    .binding(SAMPLER_BINDING_INDEX)
                    .descriptor_count(MAX_SAMPLER_RESOURCES)
                    .descriptor_type(vk::DescriptorType::SAMPLER)
                    .stage_flags(vk::ShaderStageFlags::ALL)
                    .flag(
                        vk::DescriptorBindingFlags::PARTIALLY_BOUND
                            | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                    ),
                descriptor::descriptor_set_layout_builder::DescriptorSetLayoutBinding::default()
                    .binding(SAMPLED_IMAGE_BINDING_INDEX)
                    .descriptor_count(MAX_IMAGE_RESOURCES)
                    .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                    .stage_flags(vk::ShaderStageFlags::ALL)
                    .flag(
                        vk::DescriptorBindingFlags::PARTIALLY_BOUND
                            | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                    ),
                descriptor::descriptor_set_layout_builder::DescriptorSetLayoutBinding::default()
                    .binding(STORAGE_IMAGE_BINDING_INDEX)
                    .descriptor_count(MAX_IMAGE_RESOURCES)
                    .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                    .stage_flags(vk::ShaderStageFlags::ALL)
                    .flag(
                        vk::DescriptorBindingFlags::PARTIALLY_BOUND
                            | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                    ),
                descriptor::descriptor_set_layout_builder::DescriptorSetLayoutBinding::default()
                    .binding(BUFFER_BINDING_INDEX)
                    .descriptor_count(1)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .stage_flags(vk::ShaderStageFlags::ALL)
                    .flag(
                        vk::DescriptorBindingFlags::PARTIALLY_BOUND
                            | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING,
                    ),
            ])
            .build(
                device.clone(),
                ptr::null(),
                vk::DescriptorSetLayoutCreateFlags::empty(),
                None,
            )?;
        let descriptor_set =
            descriptor::DescriptorSet::new(descriptor::DescriptorSetCreateInfo::NewSet {
                pool: &pool,
                layout: &set_layout,
                name: Some("GPU resource table descriptor set"),
            })?;
        // create a descriptor write
        let bda_buffer: resource::Buffer<A> =
            resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                device: device.clone(),
                name: Some(String::from("BDA Buffer")),
                allocator,
                size: ((MAX_BUFFER_RESOURCES as usize) * size_of::<vk::DeviceSize>()) as u64,
                memory_type: dagal::allocators::MemoryLocation::CpuToGpu,
                usage_flags: vk::BufferUsageFlags::STORAGE_BUFFER,
            })?;
        descriptor_set.write(&[descriptor::DescriptorWriteInfo::default()
            .ty(descriptor::DescriptorType::StorageBuffer)
            .binding(BUFFER_BINDING_INDEX)
            .slot(0)
            .push_descriptor(descriptor::DescriptorInfo::Buffer(
                vk::DescriptorBufferInfo {
                    buffer: unsafe { *bda_buffer.as_raw() },
                    offset: 0,
                    range: vk::WHOLE_SIZE,
                },
            ))]);

        Ok(Self {
            inner: Arc::new(RwLock::new(GPUResourceTableInner {
                pool,
                set_layout,
                descriptor_set,
                address_buffer: bda_buffer,
            })),
            device,
        })
    }

    /// Get the underlying [`VkDescriptorSet`](vk::DescriptorSet) of the GPU resource table for
    /// the BDA buffer
    pub async fn with_descriptor_set<R, F: FnOnce(&descriptor::DescriptorSet) -> R>(
        &self,
        f: F,
    ) -> Result<R> {
        let descriptor_set = &self.inner.read().await.descriptor_set;
        Ok(f(descriptor_set))
    }

    pub async fn get_descriptor_set(&self) -> vk::DescriptorSet {
        self.inner.read().await.descriptor_set.handle()
    }

    /// Get the underlying [VkDevice](ash::Device)
    pub fn get_device(&self) -> &dagal::device::LogicalDevice {
        &self.device
    }

    pub async unsafe fn get_descriptor_layout(&self) -> vk::DescriptorSetLayout {
        unsafe { *self.inner.read().await.set_layout.as_raw() }
    }
}

/// Only just need access to the bindless capabilities, but not the book keeping?
impl<A: Allocator> GPUResourceTable<A> {
    async unsafe fn insert_sampler(
        &self,
        sampler: vk::Sampler,
        image_view: Option<&resource::ImageView>,
        layout: vk::ImageLayout,
        id: u32,
    ) -> Result<()> {
        let p_image_info = vk::DescriptorImageInfo {
            sampler,
            image_view: image_view
                .and_then(|view| unsafe { Some(*view.as_raw()) })
                .unwrap_or(vk::ImageView::null()),
            image_layout: layout,
        };
        unsafe {
            self.with_descriptor_set(|descriptor_set| {
                self.device.get_handle().update_descriptor_sets(
                    &[vk::WriteDescriptorSet {
                        s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
                        p_next: ptr::null(),
                        dst_set: descriptor_set.handle(),
                        dst_binding: SAMPLER_BINDING_INDEX,
                        dst_array_element: id,
                        descriptor_count: 1,
                        descriptor_type: vk::DescriptorType::SAMPLER,
                        p_image_info: &p_image_info,
                        p_buffer_info: ptr::null(),
                        p_texel_buffer_view: ptr::null(),
                        _marker: Default::default(),
                    }],
                    &[],
                );
            })
            .await?;
        }
        Ok(())
    }

    async unsafe fn insert_image(
        &self,
        p_image_info: &vk::DescriptorImageInfo,
        image_flags: vk::ImageUsageFlags,
        id: u32,
    ) -> Result<()> {
        let mut write_infos: Vec<vk::WriteDescriptorSet> = Vec::new();
        if image_flags.contains(vk::ImageUsageFlags::SAMPLED) {
            write_infos.push(vk::WriteDescriptorSet {
                s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
                p_next: ptr::null(),
                dst_set: self.get_descriptor_set().await,
                dst_binding: SAMPLED_IMAGE_BINDING_INDEX,
                dst_array_element: id,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::SAMPLED_IMAGE,
                p_image_info,
                p_buffer_info: ptr::null(),
                p_texel_buffer_view: ptr::null(),
                _marker: Default::default(),
            });
        } else if image_flags.contains(vk::ImageUsageFlags::STORAGE) {
            write_infos.push(vk::WriteDescriptorSet {
                s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
                p_next: ptr::null(),
                dst_set: self.get_descriptor_set().await,
                dst_binding: STORAGE_IMAGE_BINDING_INDEX,
                dst_array_element: id,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
                p_image_info,
                p_buffer_info: ptr::null(),
                p_texel_buffer_view: ptr::null(),
                _marker: Default::default(),
            });
        }
        unsafe {
            self.device
                .get_handle()
                .update_descriptor_sets(write_infos.as_slice(), &[]);
        }
        Ok(())
    }
}
