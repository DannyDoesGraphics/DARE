use anyhow::Result;
use dagal::allocators::{Allocator, GPUAllocatorImpl};
use dagal::ash::vk;
use dagal::ash::vk::Queue;
use dagal::command::command_buffer::CmdBuffer;
use dagal::resource;
use dagal::traits::AsRaw;
use futures::stream::FuturesUnordered;
use std::ptr;
use std::sync::Arc;

#[derive(Debug)]
pub struct TransferBufferToBuffer<A: Allocator> {
    pub src_buffer: resource::Buffer<A>,
    pub dst_buffer: resource::Buffer<A>,
    pub src_offset: vk::DeviceSize,
    pub dst_offset: vk::DeviceSize,
    pub length: vk::DeviceSize,
}
unsafe impl<A: Allocator> Send for TransferBufferToBuffer<A> {}

/// Transfers a buffer's content to an image
///
/// If [`dst_layout`] is empty, we use [`src_layout`] again
#[derive(Debug)]
pub struct TransferBufferToImage<A: Allocator> {
    pub src_buffer: resource::Buffer<A>,
    pub dst_image: resource::Image<A>,
    pub src_offset: vk::DeviceSize,
    pub dst_offset: vk::Offset3D,
    pub extent: vk::Extent3D,
    pub src_layout: vk::ImageLayout,
    pub dst_layout: Option<vk::ImageLayout>,
}
unsafe impl<A: Allocator> Send for TransferBufferToImage<A> {}

#[derive(Debug)]
pub struct TransferImageToImage<A: Allocator> {
    pub src_image: resource::Image<A>,
    pub dst_image: resource::Image<A>,
    pub src_offset: vk::Offset3D,
    pub dst_offset: vk::Offset3D,
    pub extent: vk::Extent3D,
    pub src_layout: vk::ImageLayout,
    pub dst_layout: vk::ImageLayout,
}
unsafe impl<A: Allocator> Send for TransferImageToImage<A> {}

#[derive(Debug)]
pub enum TransferRequest<A: Allocator> {
    BufferToBuffer(TransferBufferToBuffer<A>),
    BufferToImage(TransferBufferToImage<A>),
    ImageToImage(TransferImageToImage<A>),
}

#[derive(Debug)]
pub enum TransferRequestCallback<A: Allocator> {
    Buffer {
        src_buffer: resource::Buffer<A>,
        dst_buffer: resource::Buffer<A>,
    },
    Image {
        src_buffer: resource::Buffer<A>,
        dst_image: resource::Image<A>,
    },
}

struct TransferRequestInner<A: Allocator> {
    request: TransferRequest<A>,
    callback: tokio::sync::oneshot::Sender<Result<TransferRequestCallback<A>>>,
}

#[derive(Debug)]
pub struct TransferPoolInner<A: Allocator> {
    thread: tokio::task::JoinHandle<()>,
    shutdown: Arc<tokio::sync::Notify>,
    sender: tokio::sync::mpsc::UnboundedSender<TransferRequestInner<A>>,
    gpu_staging_size: vk::DeviceSize,
    cpu_staging_size: vk::DeviceSize,
    cpu_staging_semaphores: tokio::sync::Semaphore,
}
/// Allows for quick transfers
#[derive(Debug, Clone)]
pub struct TransferPool<A: Allocator> {
    device: dagal::device::LogicalDevice,
    inner: Arc<TransferPoolInner<A>>,
    semaphore: Arc<tokio::sync::Semaphore>,
}
unsafe impl<A: Allocator> Send for TransferPool<A> {}
unsafe impl<A: Allocator> Sync for TransferPool<A> {}
impl<A: Allocator> Drop for TransferPoolInner<A> {
    fn drop(&mut self) {
        tracing::trace!("Stopping transfer pool thread");
        self.shutdown.notify_waiters();
        while !self.thread.is_finished() {}
        tracing::trace!("Stopped transfer pool thread");
    }
}

struct TransferProcessor {
    semaphore: Arc<tokio::sync::Semaphore>,
    device: dagal::device::LogicalDevice,
    queues: Arc<[dagal::device::Queue]>,
    fences: Arc<[dagal::sync::Fence]>,
    command_pools: Arc<[dagal::command::CommandPool]>,
}

async fn pick_available_queues(
    queues: &[dagal::device::Queue],
) -> (usize, tokio::sync::MutexGuard<'_, Queue>) {
    let lock_futures = queues
        .iter()
        .enumerate()
        .map(|(idx, queue)| {
            Box::pin(async move {
                let queue = queue.get_handle().lock().await;
                (idx, queue)
            }) as std::pin::Pin<Box<_>>
        })
        .collect::<Vec<_>>();

    // race
    futures::future::select_all(lock_futures).await.0
}

impl<A: Allocator + 'static> TransferPool<A> {
    pub fn new(
        device: dagal::device::LogicalDevice,
        gpu_staging_size: vk::DeviceSize,
        cpu_staging_size: vk::DeviceSize,
        queues: Vec<dagal::device::Queue>,
    ) -> Result<Self> {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<TransferRequestInner<A>>();

        let semaphore = Arc::new(tokio::sync::Semaphore::new(gpu_staging_size as usize));
        let queues = Arc::from(queues.into_boxed_slice());
        let shutdown = Arc::new(tokio::sync::Notify::new());
        let thread = {
            let semaphore = semaphore.clone();
            let device = device.clone();
            let shutdown = shutdown.clone();
            tokio::spawn(Self::process_upload_requests(
                semaphore, receiver, device, queues, shutdown,
            ))
        };
        let sf = Self {
            device: device.clone(),
            inner: Arc::new(TransferPoolInner {
                thread,
                sender,
                gpu_staging_size,
                shutdown,
                cpu_staging_semaphores: tokio::sync::Semaphore::new(cpu_staging_size as usize),
                cpu_staging_size,
            }),
            semaphore,
        };

        Ok(sf)
    }

    pub fn gpu_staging_size(&self) -> vk::DeviceSize {
        self.inner.gpu_staging_size
    }

    pub fn cpu_staging_size(&self) -> vk::DeviceSize {
        self.inner.cpu_staging_size
    }

    /// Returns (src_buffer, dst_buffer)
    pub async fn buffer_to_buffer_transfer(
        &self,
        request: TransferBufferToBuffer<A>,
    ) -> Result<(resource::Buffer<A>, resource::Buffer<A>)> {
        let (callback, receiver) =
            tokio::sync::oneshot::channel::<Result<TransferRequestCallback<A>>>();
        self.inner.sender.send(TransferRequestInner {
            request: TransferRequest::BufferToBuffer(request),
            callback,
        })?;
        match receiver.await?? {
            TransferRequestCallback::Buffer {
                src_buffer,
                dst_buffer,
            } => Ok((src_buffer, dst_buffer)),
            TransferRequestCallback::Image { .. } => {
                unimplemented!()
            }
        }
    }

    pub async fn buffer_to_image_transfer(
        &self,
        request: TransferBufferToImage<A>,
    ) -> Result<(resource::Buffer<A>, resource::Image<A>)> {
        let (callback, receiver) =
            tokio::sync::oneshot::channel::<Result<TransferRequestCallback<A>>>();
        self.inner.sender.send(TransferRequestInner {
            request: TransferRequest::BufferToImage(request),
            callback,
        })?;
        match receiver.await?? {
            TransferRequestCallback::Buffer { .. } => unimplemented!(),
            TransferRequestCallback::Image {
                src_buffer,
                dst_image,
            } => Ok((src_buffer, dst_image)),
        }
    }

    async fn process_upload_requests(
        semaphore: Arc<tokio::sync::Semaphore>,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<TransferRequestInner<A>>,
        device: dagal::device::LogicalDevice,
        queues: Arc<[dagal::device::Queue]>,
        shut_recv: Arc<tokio::sync::Notify>,
    ) {
        let gpu_staging_size = semaphore.available_permits();
        for queue in queues.iter() {
            if queue.get_queue_flags() & vk::QueueFlags::TRANSFER != vk::QueueFlags::TRANSFER {
                panic!("Got wrong queue flags");
            }
        }
        let command_pools: Arc<[dagal::command::CommandPool]> = Arc::from(
            queues
                .iter()
                .map(|queue| {
                    dagal::command::CommandPool::new(
                        dagal::command::CommandPoolCreateInfo::WithQueue {
                            device: device.clone(),
                            queue,
                            flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                        },
                    )
                    .unwrap()
                })
                .collect::<Vec<dagal::command::CommandPool>>()
                .into_boxed_slice(),
        );
        let fences: Arc<[dagal::sync::Fence]> = Arc::from(
            queues
                .iter()
                .map(|_| {
                    dagal::sync::Fence::new(device.clone(), vk::FenceCreateFlags::SIGNALED).unwrap()
                })
                .collect::<Vec<dagal::sync::Fence>>(),
        );
        let tasks: FuturesUnordered<tokio::task::JoinHandle<Result<()>>> = FuturesUnordered::new();

        loop {
            tokio::select! {
                _ = shut_recv.notified() => {
                    for task in tasks.iter() {
                        if !task.is_finished() {
                            task.abort();
                            while task.is_finished() {}
                        }
                    }
                    // wait for all fences to complete
                    for fence in fences.iter() {
                        fence.wait(u64::MAX).unwrap()
                    }
                    drop(command_pools);
                    drop(fences);
                    drop(queues);
                    break;
                }

                Some(request) = receiver.recv() => {
                    let dst_length: u64 = match &request.request {
                        TransferRequest::BufferToBuffer(req) => {
                            req.length
                        },
                        TransferRequest::BufferToImage(req) => {
                            (req.extent.width as u64 * req.extent.height as u64 * req.extent.depth as u64) * dagal::util::format::get_size_from_vk_format(&req.dst_image.format()) as u64
                        },
                        TransferRequest::ImageToImage(req) => {
                            (req.extent.width as u64 * req.extent.height as u64 * req.extent.depth as u64) * dagal::util::format::get_size_from_vk_format(&req.dst_image.format()) as u64
                        }
                    };
                    if dst_length > gpu_staging_size as u64 {
                        tracing::error!("Exceeds {dst_length} > {gpu_staging_size}");
                        request.callback.send(Err(anyhow::anyhow!("Exceeds gpu staging size {dst_length} > {gpu_staging_size}"))).unwrap();
                        continue;
                    }

                    let semaphore = semaphore.clone();
                    let device = device.clone();
                    let queues = queues.clone();
                    let command_pools = command_pools.clone();
                    let fences = fences.clone();
                    let processor = TransferProcessor {
                                semaphore,
                                device,
                                queues,
                                fences,
                                command_pools,
                            };
                    let task = tokio::spawn(Self::process_single_transfer(processor, request));
                    tasks.push(task);
                }
            }
        }
    }

    async fn process_single_transfer(
        processor: TransferProcessor,
        request: TransferRequestInner<A>,
    ) -> Result<()> {
        let src_length: vk::DeviceSize = match &request.request {
            TransferRequest::BufferToBuffer(req) => req.length as u64,
            TransferRequest::BufferToImage(req) => {
                req.extent.width as u64
                    * req.extent.height as u64
                    * req.extent.depth as u64
                    * dagal::util::format::get_size_from_vk_format(&req.dst_image.format()) as u64
            }
            TransferRequest::ImageToImage(req) => {
                req.extent.width as u64
                    * req.extent.height as u64
                    * req.extent.depth as u64
                    * dagal::util::format::get_size_from_vk_format(&req.dst_image.format()) as u64
            }
        };
        // Acquire necessary semaphore permits and select an available queue
        let permit = processor.semaphore.acquire().await?;
        let (index, queue_guard) = pick_available_queues(&processor.queues).await;
        let fence: &dagal::sync::Fence = &processor.fences[index];
        // wait for fence to be cleared
        fence.fence_await().await?;

        fence.reset()?;
        let res = {
            let command_buffer = processor.command_pools[index]
                .allocate(1)?
                .pop()
                .unwrap()
                .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
                .unwrap();
            {
                unsafe {
                    // Handle images and buffer unique transfers
                    match &request.request {
                        TransferRequest::BufferToBuffer(req) => unsafe {
                            processor.device.get_handle().cmd_copy_buffer2(
                                command_buffer.handle(),
                                &vk::CopyBufferInfo2 {
                                    s_type: vk::StructureType::COPY_BUFFER_INFO_2,
                                    p_next: ptr::null(),
                                    src_buffer: *req.src_buffer.as_raw(),
                                    dst_buffer: *req.dst_buffer.as_raw(),
                                    region_count: 1,
                                    p_regions: &vk::BufferCopy2 {
                                        s_type: vk::StructureType::BUFFER_COPY_2,
                                        p_next: ptr::null(),
                                        src_offset: req.src_offset,
                                        dst_offset: req.dst_offset,
                                        size: req.length,
                                        _marker: Default::default(),
                                    },
                                    _marker: Default::default(),
                                },
                            );
                        },
                        TransferRequest::BufferToImage(req) => {
                            processor.device.get_handle().cmd_pipeline_barrier2(
                                command_buffer.handle(),
                                &vk::DependencyInfo {
                                    s_type: vk::StructureType::DEPENDENCY_INFO,
                                    p_next: ptr::null(),
                                    dependency_flags: vk::DependencyFlags::empty(),
                                    memory_barrier_count: 0,
                                    p_memory_barriers: ptr::null(),
                                    buffer_memory_barrier_count: 0,
                                    p_buffer_memory_barriers: ptr::null(),
                                    image_memory_barrier_count: 1,
                                    p_image_memory_barriers: &vk::ImageMemoryBarrier2 {
                                        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
                                        p_next: ptr::null(),
                                        src_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                                        src_access_mask: vk::AccessFlags2::MEMORY_WRITE,
                                        dst_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                                        dst_access_mask: vk::AccessFlags2::MEMORY_WRITE | vk::AccessFlags2::MEMORY_READ,
                                        old_layout: req.src_layout,
                                        new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                        src_queue_family_index: processor.queues[index].get_index(),
                                        dst_queue_family_index: processor.queues[index].get_index(),
                                        image: *req.dst_image.as_raw(),
                                        subresource_range: resource::Image::<GPUAllocatorImpl>::image_subresource_range(vk::ImageAspectFlags::COLOR),
                                        _marker: Default::default(),
                                    },
                                    _marker: Default::default(),
                                }
                            );
                            processor.device.get_handle().cmd_copy_buffer_to_image2(
                                command_buffer.handle(),
                                &vk::CopyBufferToImageInfo2 {
                                    s_type: vk::StructureType::COPY_BUFFER_TO_IMAGE_INFO_2,
                                    p_next: ptr::null(),
                                    src_buffer: *req.src_buffer.as_raw(),
                                    dst_image: *req.dst_image.as_raw(),
                                    dst_image_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                    region_count: 1,
                                    p_regions: &vk::BufferImageCopy2 {
                                        s_type: vk::StructureType::BUFFER_IMAGE_COPY_2,
                                        p_next: ptr::null(),
                                        buffer_offset: req.src_offset,
                                        buffer_row_length: 0,
                                        buffer_image_height: 0,
                                        image_subresource: vk::ImageSubresourceLayers {
                                            aspect_mask: vk::ImageAspectFlags::COLOR,
                                            mip_level: 0,
                                            base_array_layer: 0,
                                            layer_count: 1,
                                        },
                                        image_offset: req.dst_offset,
                                        image_extent: req.extent.clone(),
                                        _marker: Default::default(),
                                    },
                                    _marker: Default::default(),
                                },
                            );
                            processor.device.get_handle().cmd_pipeline_barrier2(
                                command_buffer.handle(),
                                &vk::DependencyInfo {
                                    s_type: vk::StructureType::DEPENDENCY_INFO,
                                    p_next: ptr::null(),
                                    dependency_flags: vk::DependencyFlags::empty(),
                                    memory_barrier_count: 0,
                                    p_memory_barriers: ptr::null(),
                                    buffer_memory_barrier_count: 0,
                                    p_buffer_memory_barriers: ptr::null(),
                                    image_memory_barrier_count: 1,
                                    p_image_memory_barriers: &vk::ImageMemoryBarrier2 {
                                        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
                                        p_next: ptr::null(),
                                        src_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                                        src_access_mask: vk::AccessFlags2::MEMORY_WRITE,
                                        dst_stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
                                        dst_access_mask: vk::AccessFlags2::MEMORY_WRITE | vk::AccessFlags2::MEMORY_READ,
                                        old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                        new_layout: req.dst_layout.unwrap_or(req.src_layout),
                                        src_queue_family_index: processor.queues[index].get_index(),
                                        dst_queue_family_index: processor.queues[index].get_index(),
                                        image: *req.dst_image.as_raw(),
                                        subresource_range: resource::Image::<GPUAllocatorImpl>::image_subresource_range(vk::ImageAspectFlags::COLOR),
                                        _marker: Default::default(),
                                    },
                                    _marker: Default::default(),
                                }
                            );
                        }
                        TransferRequest::ImageToImage(req) => unsafe {
                            processor.device.get_handle().cmd_copy_image2(
                                command_buffer.handle(),
                                &vk::CopyImageInfo2 {
                                    s_type: vk::StructureType::COPY_IMAGE_INFO_2,
                                    p_next: ptr::null(),
                                    src_image: *req.src_image.as_raw(),
                                    src_image_layout: req.src_layout,
                                    dst_image: *req.dst_image.as_raw(),
                                    dst_image_layout: req.dst_layout,
                                    region_count: 1,
                                    p_regions: &vk::ImageCopy2 {
                                        s_type: vk::StructureType::IMAGE_COPY_2,
                                        p_next: ptr::null(),
                                        src_subresource: vk::ImageSubresourceLayers {
                                            aspect_mask: vk::ImageAspectFlags::COLOR,
                                            mip_level: vk::REMAINING_MIP_LEVELS,
                                            base_array_layer: 0,
                                            layer_count: vk::REMAINING_ARRAY_LAYERS,
                                        },
                                        src_offset: Default::default(),
                                        dst_subresource: Default::default(),
                                        dst_offset: Default::default(),
                                        extent: Default::default(),
                                        _marker: Default::default(),
                                    },
                                    _marker: Default::default(),
                                },
                            );
                            unimplemented!()
                        },
                    }
                }
                let command_buffer = command_buffer.end()?;
                let cmd_buffer_info = command_buffer.submit_info();
                {
                    command_buffer
                        .submit(
                            *queue_guard,
                            &[vk::SubmitInfo2 {
                                s_type: vk::StructureType::SUBMIT_INFO_2,
                                p_next: ptr::null(),
                                flags: vk::SubmitFlags::empty(),
                                wait_semaphore_info_count: 0,
                                p_wait_semaphore_infos: ptr::null(),
                                command_buffer_info_count: 1,
                                p_command_buffer_infos: &cmd_buffer_info,
                                signal_semaphore_info_count: 0,
                                p_signal_semaphore_infos: ptr::null(),
                                _marker: Default::default(),
                            }],
                            fence.handle(),
                        )
                        .map_err(|e| {
                            tracing::error!("Failed to submit transfer command: {:?}", e);
                            e
                        })
                        .unwrap();
                }
            }
            fence.fence_await().await?;
            drop(queue_guard);
            drop(permit);
            anyhow::Ok(())
        };
        match res {
            Ok(_) => match request.request {
                TransferRequest::BufferToBuffer(req) => {
                    request
                        .callback
                        .send(Ok(TransferRequestCallback::Buffer {
                            src_buffer: req.src_buffer,
                            dst_buffer: req.dst_buffer,
                        }))
                        .unwrap();
                }
                TransferRequest::BufferToImage(req) => {
                    request
                        .callback
                        .send(Ok(TransferRequestCallback::Image {
                            src_buffer: req.src_buffer,
                            dst_image: req.dst_image,
                        }))
                        .unwrap();
                }
                TransferRequest::ImageToImage(_) => {}
            },
            Err(e) => {
                tracing::error!("Failed to complete transfer: {:?}", e);
                request
                    .callback
                    .send(Err(anyhow::anyhow!("Failed to complete transfer")))
                    .unwrap();
                fence.reset()?;
            }
        }

        Ok(())
    }

    pub fn get_device(&self) -> dagal::device::LogicalDevice {
        self.device.clone()
    }
}
