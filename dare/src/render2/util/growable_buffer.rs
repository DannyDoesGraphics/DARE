use crate::prelude as dare;
use dagal::allocators::{Allocator, MemoryLocation};
use dagal::ash::vk;
use dagal::command::command_buffer::CmdBuffer;
use dagal::resource::BufferCreateInfo;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use std::mem::size_of_val;
use std::ptr;
use std::sync::Arc;

/// Growth strategy for the growable buffer
#[derive(Debug, Clone, Copy)]
pub enum GrowthStrategy {
    /// Grow by exact amount needed
    Exact,
    /// Grow by a geometric factor (1.5x, 2x, etc.)
    Geometric(f32),
    /// Grow by fixed amount
    Fixed(u64),
    /// Custom growth function
    Custom(fn(current_size: u64, required_size: u64) -> u64),
}

impl Default for GrowthStrategy {
    fn default() -> Self {
        GrowthStrategy::Geometric(1.5)
    }
}

/// Configuration for GrowableBuffer
#[derive(Debug, Clone)]
pub struct GrowableBufferConfig {
    /// Strategy for growing the buffer
    pub growth_strategy: GrowthStrategy,
    /// Minimum size for the buffer
    pub min_size: u64,
    /// Optional maximum size for the buffer
    pub max_size: Option<u64>,
    /// Alignment for buffer size
    pub alignment: u64,
    /// Whether to enable a staging pool for temporary buffers
    pub enable_staging_pool: bool,
}

impl Default for GrowableBufferConfig {
    fn default() -> Self {
        Self {
            growth_strategy: GrowthStrategy::default(),
            min_size: 1024, // 1KB minimum
            max_size: None,
            alignment: 256, // Good default for most GPU operations
            enable_staging_pool: true,
        }
    }
}

/// A growable buffer that can change size based on uploaded data
#[derive(Debug)]
pub struct GrowableBuffer<A: Allocator + 'static> {
    handle: Option<Arc<dagal::resource::Buffer<A>>>,
    device: dagal::device::LogicalDevice,
    name: Option<String>,
    allocator: A,
    /// Current allocated size
    capacity: vk::DeviceSize,
    /// Used size (logical size)
    size: vk::DeviceSize,
    memory_type: MemoryLocation,
    usage_flags: vk::BufferUsageFlags,
    config: GrowableBufferConfig,
    /// Optional staging buffer pool for reuse
    staging_pool: Vec<dagal::resource::Buffer<A>>,
}

impl<A: Allocator + 'static> GrowableBuffer<A> {
    pub fn new(handle_ci: dagal::resource::BufferCreateInfo<A>) -> anyhow::Result<Self> {
        Self::with_config(handle_ci, GrowableBufferConfig::default())
    }

    pub fn with_config(
        handle_ci: dagal::resource::BufferCreateInfo<A>,
        config: GrowableBufferConfig,
    ) -> anyhow::Result<Self> {
        // sanity check
        match &handle_ci {
            BufferCreateInfo::NewEmptyBuffer { usage_flags, .. } => {
                assert!(
                    usage_flags.contains(
                        vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::TRANSFER_DST
                    ),
                    "Expected to find TRANSFER_SRC | TRANSFER_DST, got {:?}",
                    usage_flags
                );
            }
            _ => unimplemented!(),
        }

        let initial_size = match &handle_ci {
            BufferCreateInfo::NewEmptyBuffer { size, .. } => *size,
            _ => unimplemented!(),
        };

        // Align the initial size
        let aligned_size = Self::align_size(initial_size.max(config.min_size), config.alignment);

        // Create the initial buffer with aligned size
        let modified_ci = match handle_ci {
            BufferCreateInfo::NewEmptyBuffer {
                device,
                name,
                allocator,
                memory_type,
                usage_flags,
                ..
            } => BufferCreateInfo::NewEmptyBuffer {
                device: device.clone(),
                name: name.clone(),
                allocator,
                size: aligned_size,
                memory_type,
                usage_flags,
            },
            _ => unimplemented!(),
        };

        Ok(Self {
            device: match &modified_ci {
                BufferCreateInfo::NewEmptyBuffer { device, .. } => device.clone(),
                _ => unimplemented!(),
            },
            name: match &modified_ci {
                BufferCreateInfo::NewEmptyBuffer { name, .. } => name.clone(),
                _ => unimplemented!(),
            },
            allocator: match &modified_ci {
                BufferCreateInfo::NewEmptyBuffer { allocator, .. } => (*allocator).clone(),
                _ => unimplemented!(),
            },
            capacity: aligned_size,
            size: initial_size,
            memory_type: match &modified_ci {
                BufferCreateInfo::NewEmptyBuffer { memory_type, .. } => memory_type.clone(),
                _ => unimplemented!(),
            },
            usage_flags: match &modified_ci {
                BufferCreateInfo::NewEmptyBuffer { usage_flags, .. } => usage_flags.clone(),
                _ => unimplemented!(),
            },
            handle: Some(Arc::new(dagal::resource::Buffer::new(modified_ci)?)),
            config,
            staging_pool: Vec::new(),
        })
    }

    /// Align size to the specified alignment
    fn align_size(size: u64, alignment: u64) -> u64 {
        (size + alignment - 1) / alignment * alignment
    }

    /// Calculate the new size based on growth strategy
    fn calculate_new_size(&self, required_size: u64) -> u64 {
        let new_size = match self.config.growth_strategy {
            GrowthStrategy::Exact => required_size,
            GrowthStrategy::Geometric(factor) => {
                let exponential_size = (self.capacity as f32 * factor) as u64;
                exponential_size.max(required_size)
            }
            GrowthStrategy::Fixed(increment) => {
                let fixed_size = self.capacity + increment;
                fixed_size.max(required_size)
            }
            GrowthStrategy::Custom(func) => func(self.capacity, required_size),
        };

        let aligned_size = Self::align_size(new_size, self.config.alignment);

        // Respect max_size if set
        if let Some(max_size) = self.config.max_size {
            aligned_size.min(max_size)
        } else {
            aligned_size
        }
    }

    /// Get or create a staging buffer from the pool
    fn get_staging_buffer(&mut self, size: u64) -> anyhow::Result<dagal::resource::Buffer<A>> {
        if self.config.enable_staging_pool {
            // Try to find a suitable buffer in the pool
            if let Some(index) = self
                .staging_pool
                .iter()
                .position(|buf| buf.get_size() >= size)
            {
                return Ok(self.staging_pool.remove(index));
            }
        }

        // Create a new staging buffer
        dagal::resource::Buffer::new(BufferCreateInfo::NewEmptyBuffer {
            device: self.device.clone(),
            name: Some(format!(
                "Staging {}",
                self.name.as_deref().unwrap_or("GrowableBuffer")
            )),
            allocator: &mut self.allocator,
            size,
            memory_type: MemoryLocation::CpuToGpu,
            usage_flags: vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::TRANSFER_DST,
        })
        .map_err(|e| e.into())
    }

    /// Return a staging buffer to the pool
    fn return_staging_buffer(&mut self, buffer: dagal::resource::Buffer<A>) {
        if self.config.enable_staging_pool && self.staging_pool.len() < 8 {
            // Limit pool size to prevent memory bloat
            self.staging_pool.push(buffer);
        }
        // Otherwise, buffer is dropped
    }

    /// Get the current logical size (amount of data written)
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get the current capacity (allocated size)
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Check if the buffer needs to grow for the given size
    pub fn needs_grow(&self, required_size: u64) -> bool {
        required_size > self.capacity
    }

    /// Reserve capacity for at least the given size
    pub async fn reserve(
        &mut self,
        immediate_submit: &dare::render::util::ImmediateSubmit,
        required_size: u64,
    ) -> anyhow::Result<()> {
        if self.needs_grow(required_size) {
            let new_capacity = self.calculate_new_size(required_size);
            self.grow_to_capacity(immediate_submit, new_capacity)
                .await?;
        }
        Ok(())
    }

    /// Grow to a specific capacity, preserving existing data
    async fn grow_to_capacity(
        &mut self,
        immediate_submit: &dare::render::util::ImmediateSubmit,
        new_capacity: u64,
    ) -> anyhow::Result<()> {
        if new_capacity <= self.capacity {
            return Ok(()); // No need to grow
        }

        let new_buffer = dagal::resource::Buffer::new(BufferCreateInfo::NewEmptyBuffer {
            device: self.device.clone(),
            name: self.name.clone(),
            allocator: &mut self.allocator,
            size: new_capacity,
            memory_type: self.memory_type,
            usage_flags: self.usage_flags,
        })?;

        // Copy existing data if any
        if let Some(old_buffer) = &self.handle {
            let copy_size = self.size.min(old_buffer.get_size());
            if copy_size > 0 {
                self.copy_buffer_data(immediate_submit, old_buffer, &new_buffer, copy_size)
                    .await?;
            }
        }

        self.handle = Some(Arc::new(new_buffer));
        self.capacity = new_capacity;
        Ok(())
    }

    /// Copy data between buffers with proper synchronization
    async fn copy_buffer_data(
        &self,
        immediate_submit: &dare::render::util::ImmediateSubmit,
        src_buffer: &dagal::resource::Buffer<A>,
        dst_buffer: &dagal::resource::Buffer<A>,
        size: u64,
    ) -> anyhow::Result<()> {
        immediate_submit
            .submit(
                vk::QueueFlags::TRANSFER,
                move |_queue, cmd_buffer_recording| unsafe {
                    // Pre-copy barrier: ensure source is ready for read
                    let src_barrier = vk::BufferMemoryBarrier2 {
                        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
                        p_next: ptr::null(),
                        src_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                        src_access_mask: vk::AccessFlags2::MEMORY_WRITE,
                        dst_stage_mask: vk::PipelineStageFlags2::COPY,
                        dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
                        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                        buffer: *src_buffer.as_raw(),
                        offset: 0,
                        size,
                        _marker: Default::default(),
                    };

                    cmd_buffer_recording
                        .get_device()
                        .get_handle()
                        .cmd_pipeline_barrier2(
                            *cmd_buffer_recording.as_raw(),
                            &vk::DependencyInfo {
                                s_type: vk::StructureType::DEPENDENCY_INFO,
                                p_next: ptr::null(),
                                dependency_flags: vk::DependencyFlags::empty(),
                                memory_barrier_count: 0,
                                p_memory_barriers: ptr::null(),
                                buffer_memory_barrier_count: 1,
                                p_buffer_memory_barriers: &src_barrier,
                                image_memory_barrier_count: 0,
                                p_image_memory_barriers: ptr::null(),
                                _marker: Default::default(),
                            },
                        );

                    // Perform copy
                    let buffer_copy = vk::BufferCopy2 {
                        s_type: vk::StructureType::BUFFER_COPY_2,
                        p_next: ptr::null(),
                        src_offset: 0,
                        dst_offset: 0,
                        size,
                        _marker: Default::default(),
                    };

                    cmd_buffer_recording
                        .get_device()
                        .get_handle()
                        .cmd_copy_buffer2(
                            *cmd_buffer_recording.as_raw(),
                            &vk::CopyBufferInfo2 {
                                s_type: vk::StructureType::COPY_BUFFER_INFO_2,
                                p_next: ptr::null(),
                                src_buffer: *src_buffer.as_raw(),
                                dst_buffer: *dst_buffer.as_raw(),
                                region_count: 1,
                                p_regions: &buffer_copy,
                                _marker: Default::default(),
                            },
                        );

                    // Post-copy barrier: make destination ready for use
                    let dst_barrier = vk::BufferMemoryBarrier2 {
                        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
                        p_next: ptr::null(),
                        src_stage_mask: vk::PipelineStageFlags2::COPY,
                        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                        dst_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                        dst_access_mask: vk::AccessFlags2::MEMORY_READ
                            | vk::AccessFlags2::MEMORY_WRITE,
                        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                        buffer: *dst_buffer.as_raw(),
                        offset: 0,
                        size,
                        _marker: Default::default(),
                    };

                    cmd_buffer_recording
                        .get_device()
                        .get_handle()
                        .cmd_pipeline_barrier2(
                            *cmd_buffer_recording.as_raw(),
                            &vk::DependencyInfo {
                                s_type: vk::StructureType::DEPENDENCY_INFO,
                                p_next: ptr::null(),
                                dependency_flags: vk::DependencyFlags::empty(),
                                memory_barrier_count: 0,
                                p_memory_barriers: ptr::null(),
                                buffer_memory_barrier_count: 1,
                                p_buffer_memory_barriers: &dst_barrier,
                                image_memory_barrier_count: 0,
                                p_image_memory_barriers: ptr::null(),
                                _marker: Default::default(),
                            },
                        );
                },
            )
            .await
    }

    pub fn get_buffer(&self) -> Arc<dagal::resource::Buffer<A>> {
        self.handle.as_ref().unwrap().clone()
    }

    /// Upload data to the buffer, growing it if necessary
    pub async fn upload_to_buffer<T: Sized>(
        &mut self,
        immediate_submit: &dare::render::util::ImmediateSubmit,
        items: &[T],
    ) -> anyhow::Result<()> {
        let data_size = size_of_val(items) as u64;
        if data_size == 0 {
            return Ok(());
        }

        // Reserve capacity if needed
        self.reserve(immediate_submit, data_size).await?;

        // Check if using cpu to gpu memory type to write directly
        if matches!(self.memory_type, MemoryLocation::CpuToGpu) {
            unsafe {
                self.handle.as_ref().unwrap().write_unsafe(0, items)?;
            }
        } else {
            // Use staging buffer for other memory types
            let mut staging_buffer = self.get_staging_buffer(data_size)?;
            staging_buffer.write(0, items)?;

            // Perform the upload
            immediate_submit
                .submit(vk::QueueFlags::TRANSFER, |_, cmd_buffer_recording| unsafe {
                    cmd_buffer_recording
                        .get_device()
                        .get_handle()
                        .cmd_copy_buffer2(
                            *cmd_buffer_recording.as_raw(),
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
                                    size: data_size,
                                    _marker: Default::default(),
                                },
                                _marker: Default::default(),
                            },
                        );
                })
                .await?;

            // Return staging buffer to pool
            self.return_staging_buffer(staging_buffer);
        }

        // Update logical size
        self.size = data_size;
        Ok(())
    }

    /// Upload data to the buffer at a specific offset, growing it if necessary
    pub async fn upload_to_buffer_at_offset<T: Sized>(
        &mut self,
        immediate_submit: &dare::render::util::ImmediateSubmit,
        offset: u64,
        items: &[T],
    ) -> anyhow::Result<()> {
        let data_size = size_of_val(items) as u64;
        if data_size == 0 {
            return Ok(());
        }

        // Calculate required total size including the offset
        let required_total_size = offset + data_size;

        // Reserve capacity if needed
        self.reserve(immediate_submit, required_total_size).await?;

        // Check if using cpu to gpu memory type to write directly
        if matches!(self.memory_type, MemoryLocation::CpuToGpu) {
            unsafe {
                self.handle.as_ref().unwrap().write_unsafe(offset, items)?;
            }
        } else {
            // Use staging buffer for other memory types
            let mut staging_buffer = self.get_staging_buffer(data_size)?;
            staging_buffer.write(0, items)?;

            // Perform the upload
            immediate_submit
                .submit(vk::QueueFlags::TRANSFER, |_, cmd_buffer_recording| unsafe {
                    cmd_buffer_recording
                        .get_device()
                        .get_handle()
                        .cmd_copy_buffer2(
                            *cmd_buffer_recording.as_raw(),
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
                                    dst_offset: offset,
                                    size: data_size,
                                    _marker: Default::default(),
                                },
                                _marker: Default::default(),
                            },
                        );
                })
                .await?;

            // Return staging buffer to pool
            self.return_staging_buffer(staging_buffer);
        }

        // Update logical size to accommodate the new data if it extends beyond current size
        self.size = self.size.max(required_total_size);
        Ok(())
    }
}
