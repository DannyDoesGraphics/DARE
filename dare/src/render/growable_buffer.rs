use anyhow::Result;

use dagal::allocators::{Allocator, ArcAllocator, GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;
use dagal::command::command_buffer::CmdBuffer;
use dagal::resource;
use dagal::resource::traits::{Nameable, Resource};
use dagal::traits::AsRaw;

/// Describes a [`resource::Buffer`] which can grow, but not shrink
///
/// # Growing
/// Upon calling [`GrowableBuffer::update_buffer`], the buffer will check to see if new_size > prev_size,
/// and will allocate a new buffer of new_size and copy the previous buffer over and **delete the previous
/// buffer**
#[derive(Debug)]
pub struct GrowableBuffer<A: Allocator = GPUAllocatorImpl> {
    handle: resource::Buffer<A>,
    flags: vk::BufferUsageFlags,
    location: MemoryLocation,
}

impl<A: Allocator> GrowableBuffer<A> {
    /// Same as [`resource::Buffer::new`]
    pub fn new(ci: resource::BufferCreateInfo<A>) -> Result<Self> {
        let (flags, location, device) = match &ci {
            resource::BufferCreateInfo::NewEmptyBuffer {
                usage_flags,
                memory_type,
                device,
                ..
            } => (*usage_flags, *memory_type, device.clone()),
        };
        if flags & vk::BufferUsageFlags::TRANSFER_DST != vk::BufferUsageFlags::TRANSFER_DST
            || flags & vk::BufferUsageFlags::TRANSFER_SRC != vk::BufferUsageFlags::TRANSFER_SRC
        {
            panic!("Expected vk::BufferUsageFlags::TRANSFER_DST and vk::BufferUsageFlags::TRANSFER_SRC");
        }
        let mut handle = resource::Buffer::<A>::new(ci)?;
        if let Some(debug_utils) = device.get_debug_utils().as_ref() {
            handle.set_name(debug_utils, "mesh_info_buffer")?;
        }
        Ok(Self {
            handle,
            flags,
            location,
        })
    }

    /// If the buffer size is less than the current buffer, it will not change sizes
    pub async fn update_buffer(
        &mut self,
        allocator: &mut ArcAllocator<A>,
        immediate_submit: &mut dagal::util::ImmediateSubmit,
        new_size: vk::DeviceSize,
    ) -> Result<()> {
        if self.handle.get_size() >= new_size {
            return Ok(());
        }
        // copy the buffers over
        let mut extended_buffer =
            resource::Buffer::<A>::new(resource::BufferCreateInfo::NewEmptyBuffer {
                device: self.handle.get_device().clone(),
                allocator,
                size: new_size,
                memory_type: self.location,
                usage_flags: self.flags,
            })?;
        if let Some(debug_utils) = immediate_submit
            .get_device()
            .clone()
            .get_debug_utils()
            .as_ref()
        {
            extended_buffer.set_name(debug_utils, "Mesh Info Buffer")?;
        }
        immediate_submit
            .submit(|ctx| unsafe {
                ctx.device.get_handle().cmd_copy_buffer(
                    **ctx.cmd,
                    *self.handle.as_raw(),
                    *extended_buffer.as_raw(),
                    &[vk::BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: self.handle.get_size(),
                    }],
                );
            })
            .await?;
        self.handle = extended_buffer;
        Ok(())
    }

    pub fn get_handle(&self) -> &resource::Buffer<A> {
        &self.handle
    }

    pub fn get_handle_mut(&mut self) -> &mut resource::Buffer<A> {
        &mut self.handle
    }
}
