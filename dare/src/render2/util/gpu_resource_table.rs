use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::{mem, ptr};
use tokio::sync::RwLock;

use anyhow::Result;
/// Bevy
use bevy_ecs::prelude as becs;
use dagal::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use dagal::{descriptor, resource};
/// A GPU resource table
use dare_containers::prelude as container;
use dare_containers::prelude::Container;

/// Defines the actual data backed in a resource table slot
#[derive(Debug)]
enum RTSlot<T> {
    Slot(T),
    Arc(Weak<T>),
}

#[derive(Debug)]
pub enum GPUSlot<T> {
    Slot(container::Slot<RTSlot<T>>),
    Arc(Arc<T>),
    Weak(Weak<T>),
}

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

    // Storage for the underlying resources
    images: Arc<RwLock<container::FreeList<RTSlot<resource::Image<A>>>>>,
    image_views: Arc<RwLock<container::FreeList<RTSlot<resource::ImageView>>>>,
    buffers: Arc<RwLock<container::FreeList<RTSlot<resource::Buffer<A>>>>>,
    samplers: Arc<RwLock<container::FreeList<RTSlot<resource::Sampler>>>>,

    device: dagal::device::LogicalDevice,
}
unsafe impl<A: Allocator + 'static> Send for GPUResourceTable<A> {}
unsafe impl<A: Allocator + 'static> Sync for GPUResourceTable<A> {}

const MAX_IMAGE_RESOURCES: u32 = 65536;
const MAX_BUFFER_RESOURCES: u32 = 65536;
const MAX_SAMPLER_RESOURCES: u32 = 1024;

const BUFFER_BINDING_INDEX: u32 = 3;
const STORAGE_IMAGE_BINDING_INDEX: u32 = 2;
const SAMPLED_IMAGE_BINDING_INDEX: u32 = 1;
const SAMPLER_BINDING_INDEX: u32 = 0;

pub enum ResourceInput<'a, T: Resource<'a>> {
    ResourceHandle(T),
    ResourceArc(Arc<T>),
    ResourceWeak(Weak<T>),
    ResourceCIHandle(T::CreateInfo),
    ResourceCIArc(T::CreateInfo),
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
                size: ((MAX_BUFFER_RESOURCES as usize) * mem::size_of::<vk::DeviceSize>()) as u64,
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
            images: Arc::new(RwLock::new(container::FreeList::default())),
            image_views: Arc::new(RwLock::new(container::FreeList::default())),
            buffers: Arc::new(RwLock::new(container::FreeList::default())),
            samplers: Arc::new(RwLock::new(container::FreeList::default())),
            device,
        })
    }

    /// Ensure all weak references are still valid, if not remove them
    pub async fn update(&self) -> Result<()> {
        self.buffers.write().await.retain(|buffer| match buffer {
            RTSlot::Arc(arc) => arc.upgrade().is_some(),
            _ => true,
        });
        self.images.write().await.retain(|buffer| match buffer {
            RTSlot::Arc(arc) => arc.upgrade().is_some(),
            _ => true,
        });
        self.image_views
            .write()
            .await
            .retain(|buffer| match buffer {
                RTSlot::Arc(arc) => arc.upgrade().is_some(),
                _ => true,
            });
        self.samplers.write().await.retain(|buffer| match buffer {
            RTSlot::Arc(arc) => arc.upgrade().is_some(),
            _ => true,
        });

        Ok(())
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

    /// Create a new image view
    pub async fn new_image_view<'a>(
        &self,
        image_view_ci: ResourceInput<'a, resource::ImageView>,
    ) -> Result<GPUSlot<resource::ImageView>> {
        match image_view_ci {
            ResourceInput::ResourceHandle(resource) => Ok({
                let slot = self
                    .image_views
                    .write()
                    .await
                    .insert(RTSlot::Slot(resource));
                GPUSlot::Slot(slot)
            }),
            ResourceInput::ResourceArc(resource) => Ok({
                self.image_views
                    .write()
                    .await
                    .insert(RTSlot::Arc(Arc::downgrade(&resource)));
                GPUSlot::Arc(resource)
            }),
            ResourceInput::ResourceWeak(resource) => Ok({
                self.image_views
                    .write()
                    .await
                    .insert(RTSlot::Arc(resource.clone()));
                GPUSlot::Weak(resource)
            }),
            ResourceInput::ResourceCIHandle(ci) => {
                let resource = resource::ImageView::new(ci)?;
                let slot = self
                    .image_views
                    .write()
                    .await
                    .insert(RTSlot::Slot(resource));
                Ok(GPUSlot::Slot(container::Slot::new(
                    slot.id(),
                    slot.generation(),
                )))
            }
            ResourceInput::ResourceCIArc(ci) => {
                let resource = Arc::new(resource::ImageView::new(ci)?);
                self.image_views
                    .write()
                    .await
                    .insert(RTSlot::Arc(Arc::downgrade(&resource)));
                Ok(GPUSlot::Arc(resource))
            }
        }
    }

    pub async fn free_image_view(
        &self,
        handle: container::Slot<resource::ImageView>,
    ) -> Result<RTSlot<resource::ImageView>> {
        self.image_views
            .write()
            .await
            .remove(unsafe { handle.transmute() })
    }

    pub async fn with_image_view<R, F: FnOnce(&RTSlot<resource::ImageView>) -> R>(
        &self,
        handle: &container::Slot<resource::ImageView>,
        f: F,
    ) -> Result<R> {
        Ok(self
            .image_views
            .write()
            .await
            .with_slot(unsafe { &handle.clone().transmute() }, f)?)
    }

    /// Get a new sampler
    pub async fn new_sampler<'a>(
        &self,
        sampler: ResourceInput<'a, resource::Sampler>,
    ) -> Result<GPUSlot<resource::Sampler>> {
        let res: Result<(
            GPUSlot<resource::Sampler>,
            container::Slot<RTSlot<resource::Sampler>>,
        )> = match sampler {
            ResourceInput::ResourceHandle(resource) => {
                let slot = self.samplers.write().await.insert(RTSlot::Slot(resource));
                Ok((GPUSlot::Slot(slot.clone()), slot))
            }
            ResourceInput::ResourceArc(resource) => {
                let slot: container::Slot<RTSlot<resource::Sampler>> = self
                    .samplers
                    .write()
                    .await
                    .insert(RTSlot::Arc(Arc::downgrade(&resource)));
                Ok((GPUSlot::Arc(resource), slot))
            }
            ResourceInput::ResourceWeak(resource) => {
                let slot = self
                    .samplers
                    .write()
                    .await
                    .insert(RTSlot::Arc(resource.clone()));
                Ok((GPUSlot::Weak(resource), slot))
            }
            ResourceInput::ResourceCIHandle(ci) => unsafe {
                let resource = resource::Sampler::new(ci)?;
                let slot = self.samplers.write().await.insert(RTSlot::Slot(resource));
                Ok((GPUSlot::Slot(slot.clone()), slot))
            },
            ResourceInput::ResourceCIArc(ci) => {
                let resource = Arc::new(resource::Sampler::new(ci)?);
                let slot = self
                    .samplers
                    .write()
                    .await
                    .insert(RTSlot::Arc(Arc::downgrade(&resource)));
                Ok((GPUSlot::Arc(resource), slot))
            }
        };
        let (sampler_handle, inner_slot) = res?;

        // SAFETY: this is so fucking cursed. we assume that the time of inserting literally just before
        // and now, that it is indeed safe to blindly ignore if an Arc ref is held or not
        let sampler: vk::Sampler = match &sampler_handle {
            GPUSlot::Slot(slot) => {
                self.with_sampler(slot, |s| match s {
                    RTSlot::Slot(slot) => unsafe { *slot.as_raw() },
                    RTSlot::Arc(weak) => unsafe { *weak.upgrade().unwrap().as_raw() },
                })
                .await?
            }
            GPUSlot::Arc(arc) => unsafe { *arc.as_raw() },
            GPUSlot::Weak(resource) => unsafe { *Weak::upgrade(resource).unwrap().as_raw() },
        };
        unsafe {
            self.insert_sampler(
                sampler,
                None,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                inner_slot.id() as u32,
            )
            .await?;
        }

        Ok(sampler_handle)
    }

    pub async fn with_sampler<R, F: FnOnce(&RTSlot<resource::Sampler>) -> R>(
        &self,
        sampler: &container::Slot<RTSlot<resource::Sampler>>,
        f: F,
    ) -> Result<R> {
        self.samplers.read().await.with_slot(sampler, f)
    }

    /// Free a list sampler from the gpu resource table
    pub async fn free_sampler(
        &mut self,
        sampler: container::Slot<RTSlot<resource::Sampler>>,
    ) -> Result<()> {
        self.samplers.write().await.remove(sampler)?;
        Ok(())
    }

    pub async fn new_image<'a>(
        &self,
        image_ci: ResourceInput<'a, resource::Image<A>>,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> Result<GPUSlot<resource::Image<A>>>
    where
        A: 'a,
    {
        let res: Result<(
            GPUSlot<resource::Image<A>>,
            container::Slot<RTSlot<resource::Image<A>>>,
        )> = match image_ci {
            ResourceInput::ResourceHandle(image) => unsafe {
                let slot = self.images.write().await.insert(RTSlot::Slot(image));
                Ok((GPUSlot::Slot(slot.clone()), slot))
            },
            ResourceInput::ResourceArc(image) => unsafe {
                let slot = self
                    .images
                    .write()
                    .await
                    .insert(RTSlot::Arc(Arc::downgrade(&image)));
                Ok((GPUSlot::Arc(image), slot))
            },
            ResourceInput::ResourceWeak(resource) => {
                let slot = self
                    .images
                    .write()
                    .await
                    .insert(RTSlot::Arc(resource.clone()));
                Ok((GPUSlot::Weak(resource), slot))
            }
            ResourceInput::ResourceCIHandle(image_ci) => unsafe {
                let image = resource::Image::new(image_ci)?;
                let slot = self.images.write().await.insert(RTSlot::Slot(image));
                Ok((GPUSlot::Slot(slot.clone()), slot))
            },
            ResourceInput::ResourceCIArc(handle) => unsafe {
                let image = resource::Image::new(handle)?;
                let resource = Arc::new(image);
                let slot = self
                    .images
                    .write()
                    .await
                    .insert(RTSlot::Arc(Arc::downgrade(&resource)));
                Ok((GPUSlot::Arc(resource), slot))
            },
        };
        let (image_handle, inner_slot) = res?;
        let image_flags: vk::ImageUsageFlags =
            self.images
                .read()
                .await
                .with_slot(&inner_slot, |image_slot| match image_slot {
                    RTSlot::Slot(slot) => slot.usage_flags(),
                    RTSlot::Arc(arc) => Weak::upgrade(arc).unwrap().usage_flags(),
                })?;
        unsafe {
            self.insert_image(
                &vk::DescriptorImageInfo {
                    sampler: vk::Sampler::null(),
                    image_view,
                    image_layout,
                },
                image_flags,
                inner_slot.id() as u32,
            )
            .await?;
        }
        Ok(image_handle)
    }

    pub async fn free_image(
        &mut self,
        handle: container::Slot<RTSlot<resource::Image<A>>>,
    ) -> Result<()> {
        self.images.write().await.remove(handle)?;
        Ok(())
    }

    /// Create a new buffer and put it into the bindless buffer
    ///
    /// We expect every buffer created to have a SHADER_DEVICE_ADDRESS flag enabled
    pub async fn new_buffer<'a>(
        &self,
        buffer_input: ResourceInput<'a, resource::Buffer<A>>,
    ) -> Result<GPUSlot<resource::Buffer<A>>>
    where
        A: 'a,
    {
        match buffer_input {
            ResourceInput::ResourceHandle(buffer) => {
                let buffer_address = buffer.address();
                let handle = self.buffers.write().await.insert(RTSlot::Slot(buffer));
                self.inner.write().await.address_buffer.write(
                    (mem::size_of::<vk::DeviceMemory>() * handle.id()) as vk::DeviceSize,
                    &[buffer_address],
                )?;
                Ok(GPUSlot::Slot(handle.clone()))
            }
            ResourceInput::ResourceArc(buffer) => {
                self.buffers
                    .write()
                    .await
                    .insert(RTSlot::Arc(Arc::downgrade(&buffer)));
                Ok(GPUSlot::Arc(buffer))
            }
            ResourceInput::ResourceWeak(resource) => {
                let slot = self
                    .buffers
                    .write()
                    .await
                    .insert(RTSlot::Arc(resource.clone()));
                Ok(GPUSlot::Weak(resource))
            }
            ResourceInput::ResourceCIHandle(buffer_ci) => {
                // Validate the usage flags
                if let resource::BufferCreateInfo::NewEmptyBuffer { usage_flags, .. } = buffer_ci {
                    if usage_flags & vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                        != vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    {
                        return Err(anyhow::Error::from(
                            dagal::DagalError::NoShaderDeviceAddress,
                        ));
                    }
                }

                // Create the buffer
                let buffer: resource::Buffer<A> = resource::Buffer::new(buffer_ci)?;

                // Inline the logic from ResourceHandle
                let buffer_address = buffer.address();
                let handle = self.buffers.write().await.insert(RTSlot::Slot(buffer));
                self.inner.write().await.address_buffer.write(
                    (mem::size_of::<vk::DeviceMemory>() * handle.id()) as vk::DeviceSize,
                    &[buffer_address],
                )?;
                Ok(GPUSlot::Slot(handle.clone()))
            }
            ResourceInput::ResourceCIArc(buffer_ci) => {
                // Validate the usage flags
                if let resource::BufferCreateInfo::NewEmptyBuffer { usage_flags, .. } = buffer_ci {
                    if usage_flags & vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                        != vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                    {
                        return Err(anyhow::Error::from(
                            dagal::DagalError::NoShaderDeviceAddress,
                        ));
                    }
                }

                // Create the buffer inside an Arc
                let buffer: Arc<resource::Buffer<A>> = Arc::new(resource::Buffer::new(buffer_ci)?);

                // Inline the logic from ResourceArc
                self.buffers
                    .write()
                    .await
                    .insert(RTSlot::Arc(Arc::downgrade(&buffer)));
                Ok(GPUSlot::Arc(buffer))
            }
        }
    }

    pub async fn free_buffer(
        &self,
        handle: container::Slot<RTSlot<resource::Buffer<A>>>,
    ) -> Result<()> {
        self.buffers.write().await.remove(handle)?;
        Ok(())
    }

    /// Get buffer
    pub async fn with_buffer<R, F: FnOnce(&RTSlot<resource::Buffer<A>>) -> R>(
        &self,
        handle: &container::Slot<RTSlot<resource::Buffer<A>>>,
        f: F,
    ) -> Result<R> {
        self.buffers.read().await.with_slot(handle, f)
    }

    pub async fn with_buffer_mut<R, F: FnOnce(&mut RTSlot<resource::Buffer<A>>) -> R>(
        &mut self,
        handle: &container::Slot<RTSlot<resource::Buffer<A>>>,
        f: F,
    ) -> Result<R> {
        self.buffers.write().await.with_slot_mut(handle, f)
    }

    /// Utility function to acquire device address
    pub async fn get_bda(
        &self,
        handle: &container::Slot<RTSlot<resource::Buffer<A>>>,
    ) -> Result<vk::DeviceAddress> {
        self.with_buffer(handle, |buf| match buf {
            RTSlot::Slot(buffer) => Ok(buffer.address()),
            RTSlot::Arc(buffer) => buffer
                .upgrade()
                .ok_or(dagal::DagalError::NoStrongReferences.into())
                .map(|buffer| buffer.address()),
        })
        .await?
    }

    /// Get even more images
    pub async fn with_image<R, F: FnOnce(&RTSlot<resource::Image<A>>) -> R>(
        &self,
        handle: &container::Slot<RTSlot<resource::Image<A>>>,
        f: F,
    ) -> Result<R> {
        self.images.read().await.with_slot(handle, f)
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

        Ok(())
    }

    async unsafe fn insert_image(
        &self,
        p_image_info: &vk::DescriptorImageInfo,
        image_flags: vk::ImageUsageFlags,
        id: u32,
    ) -> Result<()> {
        let mut write_infos: Vec<vk::WriteDescriptorSet> = Vec::new();
        if image_flags & vk::ImageUsageFlags::SAMPLED == vk::ImageUsageFlags::SAMPLED {
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
        }
        if image_flags & vk::ImageUsageFlags::STORAGE == vk::ImageUsageFlags::STORAGE {
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
