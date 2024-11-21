use dagal::allocators::{Allocator, ArcAllocator, MemoryLocation};
use dagal::ash::vk;
use dagal::resource::traits::Resource;
use dagal::resource::BufferCreateInfo;
use std::ops::Deref;
use std::ptr;
use std::sync::Arc;
use dagal::command::command_buffer::CmdBuffer;
use dagal::traits::AsRaw;
use crate::prelude as dare;

/// blocking changes i need to make:
/// TODO:
/// - port over [`vk::DeviceCreateInfo`] into our own custom struct to get rid of the lifetime
/// requirements
/// Describes a buffer which can grow dynamically, but shrinks rarely
pub struct GrowableBuffer<A: Allocator + 'static> {
    handle: Option<Arc<dagal::resource::Buffer<A>>>,
    device: dagal::device::LogicalDevice,
    name: Option<String>,
    allocator: ArcAllocator<A>,
    size: vk::DeviceSize,
    memory_type: MemoryLocation,
    usage_flags: vk::BufferUsageFlags,
}

impl<A: Allocator + 'static> GrowableBuffer<A> {
    pub fn new<'a>(handle_ci: dagal::resource::BufferCreateInfo<'a, A>) -> anyhow::Result<Self> {
        Ok(Self {
            device: match &handle_ci {
                BufferCreateInfo::NewEmptyBuffer { device, .. } => device.clone(),
            },
            name: match &handle_ci {
                BufferCreateInfo::NewEmptyBuffer { name, .. } => name.clone(),
            },
            allocator: match &handle_ci {
                BufferCreateInfo::NewEmptyBuffer { allocator, .. } => (*allocator).clone(),
            },
            size: match &handle_ci {
                BufferCreateInfo::NewEmptyBuffer { size, .. } => size.clone(),
            },
            memory_type: match &handle_ci {
                BufferCreateInfo::NewEmptyBuffer { memory_type, .. } => memory_type.clone(),
            },
            usage_flags: match &handle_ci {
                BufferCreateInfo::NewEmptyBuffer { usage_flags, .. } => {
                    println!("{:?}", usage_flags);
                    usage_flags.clone()
                },
            },
            handle: Some(Arc::new(dagal::resource::Buffer::new(handle_ci)?)),
        })
    }

    /// Sets the current buffer by [`dl`]
    pub async fn new_size(&mut self, immediate_submit: &dare::render::util::ImmediateSubmit, dl: i128) -> anyhow::Result<()> {
        assert!(self.size as i128 + dl > 0);
        let new_buffer = dagal::resource::Buffer::new(BufferCreateInfo::NewEmptyBuffer {
            device: self.device.clone(),
            name: self.name.clone(),
            allocator: &mut self.allocator,
            size: (self.size as i128 + dl) as vk::DeviceSize,
            memory_type: self.memory_type.clone(),
            usage_flags: self.usage_flags.clone(),
        })?;
        let old_buffer = self.handle.take().unwrap();
        // todo: implement transfer on larger size
        unsafe {
            let buffer_copy = vk::BufferCopy2 {
                s_type: vk::StructureType::BUFFER_COPY_2,
                p_next: ptr::null(),
                src_offset: 0,
                dst_offset: 0,
                size: old_buffer.get_size().min(new_buffer.get_size()),
                _marker: Default::default(),
            };
            let buffer_copy = vk::CopyBufferInfo2 {
                s_type: vk::StructureType::COPY_BUFFER_INFO_2,
                p_next: ptr::null(),
                src_buffer: unsafe { *old_buffer.as_raw() },
                dst_buffer: unsafe { *new_buffer.as_raw() },
                region_count: 1,
                p_regions: &buffer_copy,
                _marker: Default::default(),
            };
            let memory_barrier_before = vk::BufferMemoryBarrier2 {
                s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
                p_next: ptr::null(),
                src_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                src_access_mask: vk::AccessFlags2::MEMORY_WRITE,
                dst_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
                src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                buffer: unsafe { *old_buffer.as_raw() },
                offset: 0,
                size: old_buffer.get_size().min(new_buffer.get_size()),
                _marker: Default::default(),
            };
            let memory_barrier_after = vk::BufferMemoryBarrier2 {
                s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
                p_next: ptr::null(),
                src_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                dst_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                dst_access_mask: vk::AccessFlags2::MEMORY_WRITE,
                src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                buffer: unsafe { *old_buffer.as_raw() },
                offset: 0,
                size: old_buffer.get_size().min(new_buffer.get_size()),
                _marker: Default::default(),
            };
            immediate_submit.submit(move |queue, cmd_buffer_recording| unsafe {
                cmd_buffer_recording.get_device()
                    .get_handle()
                    .cmd_pipeline_barrier2(*cmd_buffer_recording.get_handle(),
                                           &vk::DependencyInfo {
                                               s_type: vk::StructureType::DEPENDENCY_INFO,
                                               p_next: ptr::null(),
                                               dependency_flags: vk::DependencyFlags::empty(),
                                               memory_barrier_count: 0,
                                               p_memory_barriers: ptr::null(),
                                               buffer_memory_barrier_count: 1,
                                               p_buffer_memory_barriers: &memory_barrier_before,
                                               image_memory_barrier_count: 0,
                                               p_image_memory_barriers: ptr::null(),
                                               _marker: Default::default(),
                                           }
                    );

                cmd_buffer_recording.get_device()
                    .get_handle()
                    .cmd_copy_buffer2(cmd_buffer_recording.handle(), &buffer_copy);

                cmd_buffer_recording.get_device()
                    .get_handle()
                    .cmd_pipeline_barrier2(cmd_buffer_recording.handle(),
                                           &vk::DependencyInfo {
                                               s_type: vk::StructureType::DEPENDENCY_INFO,
                                               p_next: ptr::null(),
                                               dependency_flags: vk::DependencyFlags::empty(),
                                               memory_barrier_count: 0,
                                               p_memory_barriers: ptr::null(),
                                               buffer_memory_barrier_count: 1,
                                               p_buffer_memory_barriers: &memory_barrier_after,
                                               image_memory_barrier_count: 0,
                                               p_image_memory_barriers: ptr::null(),
                                               _marker: Default::default(),
                                           }
                    );
            }).await?;
            self.size = (self.size as i128 + dl) as vk::DeviceSize;
            self.handle = Some(Arc::new(new_buffer));
            Ok(())
        }
    }

    pub fn get_buffer(&self) -> Arc<dagal::resource::Buffer<A>> {
        self.handle.as_ref().unwrap().clone()
    }

    pub async fn upload_to_buffer<T: Sized>(&mut self, immediate_submit: &dare::render::util::ImmediateSubmit, items: &[T]) -> anyhow::Result<()> {
        let mut staging_buffer = dagal::resource::Buffer::new(
            BufferCreateInfo::NewEmptyBuffer {
                device: self.device.clone(),
                name: Some(format!("Transfer {}", self.name.as_ref().map(|v| v.as_str()).unwrap_or("Swap buffer"))),
                allocator: &mut self.allocator,
                size: size_of_val(items) as vk::DeviceSize,
                memory_type: MemoryLocation::CpuToGpu,
                usage_flags: vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            }
        )?;
        staging_buffer.write(0, items)?;
        if self.size < size_of_val(items) as u64 {
            self.new_size(immediate_submit,size_of_val(items) as i128 - self.size as i128).await?;
        }
        immediate_submit.submit(|_, cmd_buffer_recording|
        unsafe {
            cmd_buffer_recording
                .get_device()
                .get_handle()
                .cmd_copy_buffer2(
                    *cmd_buffer_recording.get_handle(),
                    &vk::CopyBufferInfo2 {
                        s_type: vk::StructureType::COPY_BUFFER_INFO_2,
                        p_next: ptr::null(),
                        src_buffer: *staging_buffer.as_raw(),
                        dst_buffer: *self.handle.as_ref().unwrap().as_raw(),
                        region_count: 1,
                        p_regions: &vk::BufferCopy2 {
                            s_type: vk::StructureType::BUFFER_COPY_2,
                            p_next: ptr::null(),
                            src_offset: 0,
                            dst_offset: 0,
                            size: staging_buffer.get_size(),
                            _marker: Default::default(),
                        },
                        _marker: Default::default(),
                    }
                );
        }).await?;

        Ok(())
    }
}
