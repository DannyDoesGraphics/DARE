//! An implementation of the transfer belt pattern for efficient GPU resource uploads.
//!
//! Each staging chunk is submitted independently and signals a unique timeline semaphore
//! ticket. [`TransferManager::poll`] reclaims chunks once `current_value >= max_ticket`.
use bevy_ecs::prelude::*;
use dagal::ash::vk;
use dagal::{allocators::Allocator, ash::vk::Handle, resource::traits::Resource, traits::AsRaw};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::Poll;

/// Timeline semaphore shared between the belt and pollable transfer futures.
#[derive(Debug)]
struct TransferSync {
    semaphore: dagal::sync::Semaphore,
}

#[derive(Clone, Debug)]
struct TransferCompletion {
    inner: Arc<TransferCompletionInner>,
}

#[derive(Debug)]
struct TransferCompletionInner {
    sync: Arc<TransferSync>,
    ticket: AtomicU64,
}

impl TransferCompletion {
    fn new(sync: Arc<TransferSync>) -> Self {
        Self {
            inner: Arc::new(TransferCompletionInner {
                sync,
                ticket: AtomicU64::new(0),
            }),
        }
    }

    fn set_ticket(&self, ticket: u64) {
        self.inner.ticket.store(ticket, Ordering::Release);
    }

    fn poll(&self) -> Poll<dagal::Result<()>> {
        let ticket = self.inner.ticket.load(Ordering::Acquire);
        if ticket == 0 {
            return Poll::Pending;
        }
        match self.inner.sync.semaphore.current_value() {
            Ok(value) if value >= ticket => Poll::Ready(Ok(())),
            Ok(_) => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

/// Pollable handle for a transfer submitted through [`TransferPool`].
#[derive(Clone, Debug)]
pub struct TransferFuture {
    completion: TransferCompletion,
}

impl TransferFuture {
    pub fn poll(&self) -> Poll<dagal::Result<()>> {
        self.completion.poll()
    }
}

impl std::future::Future for TransferFuture {
    type Output = dagal::Result<()>;

    fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        self.completion.poll()
    }
}

/// Each chunk has a buffer backing it, along with a head cursor for sub-allocations
#[derive(Debug)]
struct Chunk<A: Allocator> {
    buffer: dagal::resource::Buffer<A>,
    /// sub-allocate cursor (ring buffer style)
    head: u64,
    /// GPU lifetime for this chunk associated with the submission #
    max_ticket: u64,
    /// Destinations to write to
    destinations: Vec<ChunkDestination>,
    /// Number of consecutive flushes where this chunk was not used
    unused_flush_count: u64,
}

/// Destination for each chunk slice
#[derive(Debug)]
enum ChunkDestination {
    Buffer {
        src_queue_family: u32,
        /// If None, same as src_queue_family
        dst_queue_family: Option<u32>,
        buffer: vk::Buffer,
        dst_offset: u64,
        src_offset: u64,
        src_size: u64,
        completion: Option<TransferCompletion>,
    },
    Image {
        src_queue_family: u32,
        /// If None, same as src_queue_family
        dst_queue_family: Option<u32>,
        image: vk::Image,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        dst_offset: vk::Offset3D,
        dst_extent: vk::Extent3D,
        subresource_layers: vk::ImageSubresourceLayers,
        src_offset: u64,
        src_size: u64,
        completion: Option<TransferCompletion>,
    },
}

/// Transfer request for buffer or image
#[derive(Debug)]
pub enum TransferRequest {
    Buffer {
        dst_queue_family: Option<u32>,
        buffer: vk::Buffer,
        dst_offset: u64,
        src_size: u64,
        data: Box<[u8]>,
        completion: Option<TransferCompletion>,
    },
    Image {
        dst_queue_family: Option<u32>,
        image: vk::Image,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        dst_offset: vk::Offset3D,
        dst_extent: vk::Extent3D,
        subresource_layers: vk::ImageSubresourceLayers,
        src_size: u64,
        data: Box<[u8]>,
        completion: Option<TransferCompletion>,
    },
}

impl TransferRequest {
    /// Size in bytes to be transferred
    pub fn size(&self) -> u64 {
        match self {
            TransferRequest::Buffer { src_size, .. } => *src_size,
            TransferRequest::Image { src_size, .. } => *src_size,
        }
    }

    fn attach_completion(&mut self, completion: TransferCompletion) {
        match self {
            Self::Buffer { completion: slot, .. } | Self::Image { completion: slot, .. } => {
                debug_assert!(slot.is_none());
                *slot = Some(completion);
            }
        }
    }
}

impl ChunkDestination {
    /// Size in bytes to be transferred
    pub fn size(&self) -> u64 {
        match self {
            ChunkDestination::Buffer { src_size, .. } => *src_size,
            ChunkDestination::Image { src_size, .. } => *src_size,
        }
    }
}

/**
 * 2nd generation transfer manager
 *
 * Uses a belt allocator to stream transfers onto the CPU <-> GPU
 *
 * # Chunk
 * - An allocation of contiguous memory
 *
 * # Slice
 * - A sub-allocation of a chunk that actually contains the transfer data
 */
#[derive(Debug, bevy_ecs::resource::Resource)]
pub struct TransferManager<A: Allocator> {
    /// Active chunks are actively being written to from host
    chunks_active: Vec<Chunk<A>>,
    /// Upon submission, active chunks are closed and moved to closed to start device processing
    chunks_closed: Vec<Chunk<A>>,
    /// Free chunks are available after the device has finished processing them
    chunks_free: Vec<Chunk<A>>,

    queue: dagal::device::Queue,
    command_pool: dagal::command::CommandPool,
    allocator: A,

    sync: Arc<TransferSync>,
    next_ticket: u64,

    /// Max belt size of all chunks
    max_belt_size: u64,

    /// Queue for transfer requests
    send: std::sync::mpsc::Sender<TransferRequest>,
    recv: std::sync::mpsc::Receiver<TransferRequest>,

    /// LRU cache size
    lru_cache: u64,
}

impl<A: Allocator> TransferManager<A> {
    pub fn new(
        queue: dagal::device::Queue,
        allocator: A,
        max_belt_size: u64,
        lru_cache: u64,
    ) -> dagal::Result<Self> {
        let device = allocator.get_device().clone();
        let command_pool = dagal::command::CommandPool::new(
            dagal::command::CommandPoolCreateInfo::WithQueue {
                device: device.clone(),
                flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                queue: &queue,
            },
        )?;

        // Queue for transfers
        let (send, recv) = std::sync::mpsc::channel::<TransferRequest>();
        let sync = Arc::new(TransferSync {
            semaphore: dagal::sync::Semaphore::new(
                vk::SemaphoreCreateFlags::empty(),
                device.clone(),
                0,
            )?,
        });
        Ok(Self {
            chunks_active: Vec::new(),
            chunks_closed: Vec::new(),
            chunks_free: Vec::new(),
            queue,
            command_pool,
            allocator,

            sync,
            next_ticket: 1,

            max_belt_size,
            send,
            recv,

            lru_cache,
        })
    }

    /// Return an active chunk with enough space for the requested allocation
    fn find_active_with_space(&self, size: u64) -> Option<usize> {
        for (i, chunk) in self.chunks_active.iter().enumerate() {
            if chunk.head + size <= chunk.buffer.get_size() {
                return Some(i);
            }
        }
        None
    }

    /// Create a new chunk in the belt buffer which is sent into the free list
    fn create_chunk(&mut self, min_size: u64) -> dagal::Result<()> {
        let size = self.max_belt_size.max(min_size);
        let buffer =
            dagal::resource::Buffer::new(dagal::resource::BufferCreateInfo::NewEmptyBuffer {
                device: self.allocator.get_device().clone(),
                name: Some("TransferManager2 Chunk Buffer".to_string()),
                allocator: &mut self.allocator,
                size,
                memory_type: dagal::allocators::MemoryLocation::CpuToGpu,
                usage_flags: vk::BufferUsageFlags::TRANSFER_SRC
                    | vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            })?;
        let chunk = Chunk {
            buffer,
            head: 0,
            max_ticket: 0,
            destinations: Vec::new(),
            unused_flush_count: 0,
        };
        self.chunks_free.push(chunk);

        Ok(())
    }

    /// Poll the belt at frame start: reclaim staging chunks whose GPU work has finished.
    pub fn poll(&mut self) -> dagal::Result<()> {
        self.reclaim()
    }

    /// Move closed -> free chunks if their device work is done
    /// Also handles LRU eviction of unused chunks
    fn reclaim(&mut self) -> dagal::Result<()> {
        // Try to reclaim closed chunks
        let value: u64 = self.sync.semaphore.current_value()?;
        // iterate all closed chunks and move to free if done
        let mut ix: usize = 0;
        while ix < self.chunks_closed.len() {
            if self.chunks_closed[ix].max_ticket <= value {
                let mut chunk: Chunk<A> = self.chunks_closed.remove(ix);
                chunk.head = 0; // belt can start at the front again once freed
                chunk.max_ticket = 0;
                chunk.destinations.clear();
                self.chunks_free.push(chunk);
            } else {
                ix += 1;
            }
        }

        // LRU eviction
        self.chunks_free.retain_mut(|chunk| {
            chunk.unused_flush_count += 1;
            chunk.unused_flush_count <= self.lru_cache
        });

        Ok(())
    }

    /// Retrieve a slice of the buffer for the given allocation
    ///
    /// Returns an index from the [`Self::chunks_active`] list
    fn allocate_slice(&mut self, size: u64) -> dagal::Result<usize> {
        self.reclaim()?;
        if let Some(chunk_ix) = self.find_active_with_space(size) {
            Ok(chunk_ix)
        } else {
            // No active chunk has enough space, try to use a free chunk or create a new one
            if let Some(mut chunk) = self.chunks_free.pop() {
                chunk.unused_flush_count = 0;
                self.chunks_active.push(chunk);
                Ok(self.chunks_active.len() - 1)
            } else {
                // Create a new chunk
                self.create_chunk(size)?;
                let mut chunk = self.chunks_free.pop().unwrap();
                chunk.unused_flush_count = 0;
                self.chunks_active.push(chunk);
                Ok(self.chunks_active.len() - 1)
            }
        }
    }

    /// Record copies for one staging chunk, submit, and signal a unique timeline ticket.
    fn submit_chunk(&mut self, mut chunk: Chunk<A>) -> dagal::Result<()> {
        if chunk.destinations.is_empty() {
            chunk.head = 0;
            self.chunks_free.push(chunk);
            return Ok(());
        }

        let ticket = self.next_ticket;
        chunk.max_ticket = ticket;
        let mut completions: Vec<TransferCompletion> = Vec::new();

        let command_buffer: dagal::command::CommandBuffer =
            self.command_pool.allocate(1)?.pop().unwrap();
        let command_buffer: dagal::command::CommandBufferRecording = command_buffer
            .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .unwrap();

        let mut buffer_copy_maps: HashMap<u64, Vec<vk::BufferCopy2>> = HashMap::new();
        let mut buffer_acquire_barriers: Vec<vk::BufferMemoryBarrier2> = Vec::new();
        let mut buffer_release_barriers: Vec<vk::BufferMemoryBarrier2> = Vec::new();
        let mut image_copy_maps: HashMap<u64, Vec<vk::BufferImageCopy2>> = HashMap::new();
        let mut image_acquire_barriers: Vec<vk::ImageMemoryBarrier2> = Vec::new();
        let mut image_release_barriers: Vec<vk::ImageMemoryBarrier2> = Vec::new();

        for destination in chunk.destinations.drain(..) {
            match destination {
                ChunkDestination::Buffer {
                    src_queue_family,
                    dst_queue_family,
                    buffer,
                    dst_offset,
                    src_offset,
                    src_size,
                    completion,
                } => {
                    buffer_copy_maps.entry(buffer.as_raw()).or_default().push(
                        vk::BufferCopy2 {
                            s_type: vk::StructureType::BUFFER_COPY_2,
                            p_next: std::ptr::null(),
                            src_offset,
                            dst_offset,
                            size: src_size,
                            _marker: std::marker::PhantomData,
                        },
                    );
                    let transfer_family = self.queue.get_family_index();
                    let dst_family = dst_queue_family.unwrap_or(src_queue_family);
                    if src_queue_family != transfer_family {
                        buffer_acquire_barriers.push(vk::BufferMemoryBarrier2 {
                            s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
                            p_next: std::ptr::null(),
                            src_access_mask: vk::AccessFlags2::empty(),
                            src_stage_mask: vk::PipelineStageFlags2::TOP_OF_PIPE,
                            dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                            dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                            src_queue_family_index: src_queue_family,
                            dst_queue_family_index: transfer_family,
                            buffer,
                            offset: dst_offset,
                            size: src_size,
                            _marker: std::marker::PhantomData,
                        });
                    }
                    if dst_family != transfer_family {
                        buffer_release_barriers.push(vk::BufferMemoryBarrier2 {
                            s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
                            p_next: std::ptr::null(),
                            src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                            src_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                            dst_access_mask: vk::AccessFlags2::empty(),
                            dst_stage_mask: vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
                            src_queue_family_index: transfer_family,
                            dst_queue_family_index: dst_family,
                            buffer,
                            offset: dst_offset,
                            size: src_size,
                            _marker: std::marker::PhantomData,
                        });
                    }
                    if let Some(completion) = completion {
                        completions.push(completion);
                    }
                }
                ChunkDestination::Image {
                    src_queue_family,
                    dst_queue_family,
                    image,
                    old_layout,
                    new_layout,
                    dst_offset,
                    dst_extent,
                    subresource_layers,
                    src_offset,
                    src_size: _,
                    completion,
                } => {
                    let dst_queue_family: u32 = dst_queue_family.unwrap_or(src_queue_family);
                    let subresource_range = vk::ImageSubresourceRange {
                        aspect_mask: subresource_layers.aspect_mask,
                        base_mip_level: subresource_layers.mip_level,
                        level_count: 1,
                        base_array_layer: subresource_layers.base_array_layer,
                        layer_count: subresource_layers.layer_count,
                    };
                    image_acquire_barriers.push(vk::ImageMemoryBarrier2 {
                        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
                        p_next: std::ptr::null(),
                        src_access_mask: vk::AccessFlags2::empty(),
                        src_stage_mask: vk::PipelineStageFlags2::TOP_OF_PIPE,
                        dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                        dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                        old_layout,
                        new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        src_queue_family_index: src_queue_family,
                        dst_queue_family_index: self.queue.get_family_index(),
                        image,
                        subresource_range,
                        _marker: std::marker::PhantomData,
                    });
                    image_release_barriers.push(vk::ImageMemoryBarrier2 {
                        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
                        p_next: std::ptr::null(),
                        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                        src_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                        dst_access_mask: vk::AccessFlags2::empty(),
                        dst_stage_mask: vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
                        old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        new_layout,
                        src_queue_family_index: self.queue.get_family_index(),
                        dst_queue_family_index: dst_queue_family,
                        image,
                        subresource_range,
                        _marker: std::marker::PhantomData,
                    });
                    image_copy_maps.entry(image.as_raw()).or_default().push(
                        vk::BufferImageCopy2 {
                            s_type: vk::StructureType::BUFFER_IMAGE_COPY_2,
                            p_next: std::ptr::null(),
                            buffer_offset: src_offset,
                            buffer_row_length: 0,
                            buffer_image_height: 0,
                            image_subresource: subresource_layers,
                            image_offset: dst_offset,
                            image_extent: dst_extent,
                            _marker: std::marker::PhantomData,
                        },
                    );
                    if let Some(completion) = completion {
                        completions.push(completion);
                    }
                }
            }
        }

        if !buffer_acquire_barriers.is_empty() || !image_acquire_barriers.is_empty() {
            unsafe {
                self.allocator.get_device().get_handle().cmd_pipeline_barrier2(
                    *command_buffer.as_raw(),
                    &vk::DependencyInfo {
                        s_type: vk::StructureType::DEPENDENCY_INFO,
                        p_next: std::ptr::null(),
                        dependency_flags: vk::DependencyFlags::empty(),
                        memory_barrier_count: 0,
                        p_memory_barriers: std::ptr::null(),
                        buffer_memory_barrier_count: buffer_acquire_barriers.len() as u32,
                        p_buffer_memory_barriers: if buffer_acquire_barriers.is_empty() {
                            std::ptr::null()
                        } else {
                            buffer_acquire_barriers.as_ptr()
                        },
                        image_memory_barrier_count: image_acquire_barriers.len() as u32,
                        p_image_memory_barriers: if image_acquire_barriers.is_empty() {
                            std::ptr::null()
                        } else {
                            image_acquire_barriers.as_ptr()
                        },
                        _marker: std::marker::PhantomData,
                    },
                );
            }
        }

        for (dst_handle, copies) in buffer_copy_maps.iter() {
            unsafe {
                self.allocator.get_device().get_handle().cmd_copy_buffer2(
                    *command_buffer.as_raw(),
                    &vk::CopyBufferInfo2 {
                        s_type: vk::StructureType::COPY_BUFFER_INFO_2,
                        p_next: std::ptr::null(),
                        src_buffer: *chunk.buffer.as_raw(),
                        dst_buffer: vk::Buffer::from_raw(*dst_handle),
                        region_count: copies.len() as u32,
                        p_regions: copies.as_ptr(),
                        _marker: std::marker::PhantomData,
                    },
                );
            }
        }

        for (dst_handle, copies) in image_copy_maps.iter() {
            unsafe {
                self.allocator
                    .get_device()
                    .get_handle()
                    .cmd_copy_buffer_to_image2(
                        *command_buffer.as_raw(),
                        &vk::CopyBufferToImageInfo2 {
                            s_type: vk::StructureType::COPY_BUFFER_TO_IMAGE_INFO_2,
                            p_next: std::ptr::null(),
                            src_buffer: *chunk.buffer.as_raw(),
                            dst_image: vk::Image::from_raw(*dst_handle),
                            dst_image_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                            region_count: copies.len() as u32,
                            p_regions: copies.as_ptr(),
                            _marker: std::marker::PhantomData,
                        },
                    );
            }
        }

        if !buffer_release_barriers.is_empty() || !image_release_barriers.is_empty() {
            unsafe {
                self.allocator.get_device().get_handle().cmd_pipeline_barrier2(
                    *command_buffer.as_raw(),
                    &vk::DependencyInfo {
                        s_type: vk::StructureType::DEPENDENCY_INFO,
                        p_next: std::ptr::null(),
                        dependency_flags: vk::DependencyFlags::empty(),
                        memory_barrier_count: 0,
                        p_memory_barriers: std::ptr::null(),
                        buffer_memory_barrier_count: buffer_release_barriers.len() as u32,
                        p_buffer_memory_barriers: if buffer_release_barriers.is_empty() {
                            std::ptr::null()
                        } else {
                            buffer_release_barriers.as_ptr()
                        },
                        image_memory_barrier_count: image_release_barriers.len() as u32,
                        p_image_memory_barriers: if image_release_barriers.is_empty() {
                            std::ptr::null()
                        } else {
                            image_release_barriers.as_ptr()
                        },
                        _marker: std::marker::PhantomData,
                    },
                );
            }
        }

        let command_buffer: dagal::command::CommandBufferExecutable = command_buffer.end().unwrap();
        unsafe {
            let submit_info = vk::SubmitInfo2 {
                s_type: vk::StructureType::SUBMIT_INFO_2,
                p_next: std::ptr::null(),
                wait_semaphore_info_count: 0,
                p_wait_semaphore_infos: std::ptr::null(),
                command_buffer_info_count: 1,
                flags: vk::SubmitFlags::empty(),
                p_command_buffer_infos: &vk::CommandBufferSubmitInfo {
                    s_type: vk::StructureType::COMMAND_BUFFER_SUBMIT_INFO,
                    p_next: std::ptr::null(),
                    command_buffer: *command_buffer.as_raw(),
                    device_mask: 0,
                    _marker: std::marker::PhantomData,
                },
                signal_semaphore_info_count: 1,
                p_signal_semaphore_infos: &vk::SemaphoreSubmitInfo {
                    s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
                    p_next: std::ptr::null(),
                    semaphore: *self.sync.semaphore.as_raw(),
                    value: ticket,
                    stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                    device_index: 0,
                    _marker: std::marker::PhantomData,
                },
                _marker: std::marker::PhantomData,
            };
            command_buffer
                .submit(*self.queue.as_raw(), &[submit_info], vk::Fence::null())
                .unwrap();
        }

        for completion in completions {
            completion.set_ticket(ticket);
        }

        self.chunks_closed.push(chunk);
        self.next_ticket += 1;
        Ok(())
    }

    /// Flush any pending transfers from host to device
    pub fn flush(&mut self) -> dagal::Result<()> {
        // flush queue
        let requests: Vec<TransferRequest> = self.recv.try_iter().collect();
        for request in requests {
            // Allocate a slice for this request
            let size: u64 = request.size();
            let chunk_ix: usize = self.allocate_slice(size)?;
            let chunk: &mut Chunk<A> = &mut self.chunks_active[chunk_ix];
            let write_offset = chunk.head;
            match request {
                TransferRequest::Buffer {
                    data,
                    buffer,
                    src_size,
                    dst_offset,
                    dst_queue_family,
                    completion,
                } => {
                    chunk.buffer.write(write_offset, &data)?;
                    chunk.destinations.push(ChunkDestination::Buffer {
                        src_queue_family: self.queue.get_family_index(),
                        dst_queue_family,
                        buffer,
                        dst_offset,
                        src_offset: write_offset,
                        src_size,
                        completion,
                    });
                }
                TransferRequest::Image {
                    dst_queue_family,
                    image,
                    old_layout,
                    new_layout,
                    dst_offset,
                    dst_extent,
                    subresource_layers,
                    src_size,
                    data,
                    completion,
                } => {
                    chunk.buffer.write(write_offset, &data)?;
                    chunk.destinations.push(ChunkDestination::Image {
                        src_queue_family: self.queue.get_family_index(),
                        dst_queue_family,
                        image,
                        old_layout,
                        new_layout,
                        dst_offset,
                        dst_extent,
                        subresource_layers,
                        src_offset: write_offset,
                        src_size,
                        completion,
                    });
                }
            }
            // Advance head
            chunk.head += size;
        }

        let chunks: Vec<Chunk<A>> = self.chunks_active.drain(..).collect();
        for chunk in chunks {
            self.submit_chunk(chunk)?;
        }
        Ok(())
    }

    /// Transfer bytes. Handles creation of staging buffer -> writing to staging buffer -> transfer
    pub fn transfer_bytes_to_buffer(&mut self, bytes: &[u8]) -> dagal::Result<()> {
        let size: u64 = bytes.len() as u64;
        let chunk_ix: usize = self.allocate_slice(size)?;
        let chunk = &mut self.chunks_active[chunk_ix];
        let start: u64 = chunk.head;
        chunk.buffer.write(start, bytes)?;
        chunk.head += size;
        Ok(())
    }

    /// Get a handle to the transfer pool for sending transfer requests
    pub fn get_transfer_pool(&self) -> TransferPool {
        TransferPool {
            send: self.send.clone(),
            sync: Arc::clone(&self.sync),
        }
    }
}

/// Handle for sending transfer requests from any thread.
#[derive(Clone, Resource)]
pub struct TransferPool {
    send: std::sync::mpsc::Sender<TransferRequest>,
    sync: Arc<TransferSync>,
}

impl std::fmt::Debug for TransferPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransferPool").finish_non_exhaustive()
    }
}

impl TransferPool {
    /// Enqueue a transfer and return a future to poll for completion.
    pub fn enqueue(&self, mut transfer_request: TransferRequest) -> TransferFuture {
        let completion = TransferCompletion::new(Arc::clone(&self.sync));
        let future = TransferFuture {
            completion: completion.clone(),
        };
        transfer_request.attach_completion(completion);
        self.send.send(transfer_request).unwrap();
        future
    }

    /// Enqueue and spin until the transfer completes.
    pub fn send(&self, transfer_request: TransferRequest) -> dagal::Result<()> {
        let future = self.enqueue(transfer_request);
        loop {
            match future.poll() {
                Poll::Ready(result) => return result,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }
}
