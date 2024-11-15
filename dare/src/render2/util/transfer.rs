use anyhow::Result;
use dagal::allocators::{Allocator, ArcAllocator};
use dagal::ash::vk;
use dagal::ash::vk::{Handle, Queue};
use dagal::command::command_buffer::CmdBuffer;
use dagal::resource;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use futures::FutureExt;
use std::ptr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum TransferRequest<A: Allocator> {
    Buffer {
        src_buffer: resource::Buffer<A>,
        dst_buffer: resource::Buffer<A>,
        src_offset: vk::DeviceSize,
        dst_offset: vk::DeviceSize,
        length: vk::DeviceSize,
    },
    Image {
        src_buffer: resource::Buffer<A>,
        src_offset: vk::DeviceSize,
        src_length: vk::DeviceSize,
        extent: vk::Extent3D,
        dst_image: resource::Image<A>,
        dst_offset: vk::Offset3D,
        dst_length: vk::DeviceSize,
    },
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

pub struct TransferRequestInner<A: Allocator> {
    request: TransferRequest<A>,
    callback: tokio::sync::oneshot::Sender<anyhow::Result<TransferRequestCallback<A>>>,
}

#[derive(Debug)]
pub struct TransferPoolInner<A: Allocator> {
    thread: tokio::task::JoinHandle<()>,
    shutdown: Arc<tokio::sync::Notify>,
    sender: tokio::sync::mpsc::Sender<TransferRequestInner<A>>,
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
    }
}

struct TransferProcessor<A: Allocator> {
    semaphore: Arc<tokio::sync::Semaphore>,
    device: dagal::device::LogicalDevice,
    queues: Arc<[dagal::device::Queue]>,
    fences: Arc<[dagal::sync::Fence]>,
    command_pools: Arc<[dagal::command::CommandPool]>,
    request: TransferRequestInner<A>,
}

async fn pick_available_queues(
    queues: &[dagal::device::Queue],
) -> (usize, tokio::sync::MutexGuard<'_, Queue>) {
    loop {
        for (index, queue) in queues.iter().enumerate() {
            if let Ok(lock) = queue.get_handle().try_lock() {
                // If we successfully locked a CommandBuffer, return it
                return (index, lock);
            }
        }
        // Sleep briefly before retrying to avoid busy waiting
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

impl<A: Allocator + 'static> TransferPool<A> {
    pub fn new(
        device: dagal::device::LogicalDevice,
        cpu_staging_size: vk::DeviceSize,
        gpu_staging_size: vk::DeviceSize,
        queues: Vec<dagal::device::Queue>,
    ) -> Result<Self> {
        let (sender, receiver) = tokio::sync::mpsc::channel::<TransferRequestInner<A>>(32);

        let semaphore = Arc::new(tokio::sync::Semaphore::new(cpu_staging_size as usize));
        let queues = Arc::from(queues.into_boxed_slice());
        let shutdown = Arc::new(tokio::sync::Notify::new());
        let thread = {
            let semaphore = semaphore.clone();
            let device = device.clone();
            let shutdown = shutdown.clone();
            tokio::spawn(async move {
                Self::process_upload_requests(semaphore, receiver, device, queues, shutdown)
                    .await
                    .unwrap();
            })
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

    pub fn cpu_available_semaphore(&self) -> vk::DeviceSize {
        self.inner.cpu_staging_semaphores.available_permits() as vk::DeviceSize
    }

    pub fn cpu_acquire_semaphores(&self, semaphores: u32) -> Option<tokio::sync::SemaphorePermit> {
        self.inner
            .cpu_staging_semaphores
            .try_acquire_many(semaphores)
            .ok()
    }

    pub async fn cpu_acquire_semaphores_await(
        &self,
        semaphores: u32,
    ) -> anyhow::Result<tokio::sync::SemaphorePermit> {
        Ok(self
            .inner
            .cpu_staging_semaphores
            .acquire_many(semaphores)
            .await?)
    }

    /// Submit a transfer request to be transferred onto the gpu
    pub async fn transfer_gpu(
        &self,
        request: TransferRequest<A>,
    ) -> Result<TransferRequestCallback<A>> {
        let (sender, receiver) =
            tokio::sync::oneshot::channel::<Result<TransferRequestCallback<A>>>();
        self.inner
            .sender
            .send(TransferRequestInner {
                request,
                callback: sender,
            })
            .await?;
        receiver.await?
    }

    async fn process_upload_requests(
        semaphore: Arc<tokio::sync::Semaphore>,
        mut receiver: tokio::sync::mpsc::Receiver<TransferRequestInner<A>>,
        device: dagal::device::LogicalDevice,
        queues: Arc<[dagal::device::Queue]>,
        mut shut_recv: Arc<tokio::sync::Notify>,
    ) -> Result<()> {
        for queue in queues.iter() {
            if queue.get_queue_flags() & vk::QueueFlags::TRANSFER != vk::QueueFlags::TRANSFER {
                return Err(anyhow::anyhow!(
                    "Expected a queue with TRANSFER, got bit flag OTHER"
                ));
            }
        }
        let command_pools: Arc<[dagal::command::CommandPool]> = Arc::from(
            queues
                .iter()
                .map(|queue| {
                    dagal::command::CommandPool::new(
                        device.clone(),
                        queue,
                        vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
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
        let mut tasks: Vec<tokio::task::JoinHandle<Result<()>>> = Vec::new();

        loop {
            tokio::select! {
                _ = shut_recv.notified() => {
                    for task in tasks.iter() {
                        if task.is_finished() {
                            task.abort();
                            while task.is_finished() {}
                        }
                    }
                    // wait for all fences to complete
                    for fence in fences.iter() {
                        fence.wait(u64::MAX)?
                    }
                    drop(command_pools);
                    drop(fences);
                    drop(queues);
                    break;
                }
                Some(request) = receiver.recv() => {
                    // validate request is sane
                    if match &request.request {
                        TransferRequest::Buffer {
                            src_buffer,
                            dst_buffer,
                            src_offset,
                            dst_offset,
                            length,
                        } => dst_buffer.get_size() < *dst_offset + *length,
                        TransferRequest::Image {
                            src_buffer,
                            src_offset,
                            src_length,
                            extent,
                            dst_image,
                            dst_offset,
                            dst_length,
                        } => false,
                    } {
                        tracing::error!("Cannot transfer due to malformed request");
                        request
                            .callback
                            .send(Err(anyhow::anyhow!("Malformed request")))
                            .unwrap();
                        continue;
                    }

                    let semaphore = semaphore.clone();
                    let device = device.clone();
                    let queues = queues.clone();
                    let command_pools = command_pools.clone();
                    let fences = fences.clone();
                    let task = tokio::spawn(Self::process_single_transfer(TransferProcessor {
                        semaphore,
                        device,
                        queues,
                        fences,
                        command_pools,
                        request
                    }));
                    tasks.push(task);
                }
            }
        }
        tracing::trace!("Closing transfer");
        Ok(())
    }

    async fn process_single_transfer(processor: TransferProcessor<A>) -> Result<()> {
        // Acquire necessary semaphore permits and select an available queue
        let permits = processor
            .semaphore
            .acquire_many(match &processor.request.request {
                TransferRequest::Buffer { length, .. } => *length,
                TransferRequest::Image { dst_length, .. } => *dst_length,
            } as u32)
            .await?;
        let (index, queue_guard) = pick_available_queues(&processor.queues).await;
        let fence: &dagal::sync::Fence = &processor.fences[index];
        // wait for fence to be cleared
        fence.fence_await().await?;
        fence.reset()?;
        let command_buffer = processor.command_pools[index]
            .allocate(1)?
            .pop()
            .unwrap()
            .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .unwrap();
        {
            unsafe {
                // Handle images and buffer unique transfers
                match &processor.request.request {
                    TransferRequest::Buffer {
                        src_buffer,
                        dst_buffer,
                        src_offset,
                        dst_offset,
                        length,
                    } => {
                        processor.device.get_handle().cmd_copy_buffer2(
                            command_buffer.handle(),
                            &vk::CopyBufferInfo2 {
                                s_type: vk::StructureType::COPY_BUFFER_INFO_2,
                                p_next: ptr::null(),
                                src_buffer: unsafe { *src_buffer.as_raw() },
                                dst_buffer: unsafe { *dst_buffer.as_raw() },
                                region_count: 1,
                                p_regions: &vk::BufferCopy2 {
                                    s_type: vk::StructureType::BUFFER_COPY_2,
                                    p_next: ptr::null(),
                                    src_offset: *src_offset,
                                    dst_offset: *dst_offset,
                                    size: *length,
                                    _marker: Default::default(),
                                },
                                _marker: Default::default(),
                            },
                        );
                        unsafe {
                            println!(
                                "Staging of: {:?} -> [{}, {}) to [{}, {})",
                                *src_buffer.as_raw(),
                                *src_offset,
                                *src_offset + *length,
                                *dst_offset,
                                *dst_offset + *length,
                            );
                        }
                    }
                    TransferRequest::Image {
                        src_buffer,
                        src_offset,
                        src_length,
                        extent,
                        dst_image,
                        dst_offset,
                        dst_length,
                    } => {
                        processor.device.get_handle().cmd_copy_buffer_to_image2(
                            command_buffer.handle(),
                            &vk::CopyBufferToImageInfo2 {
                                s_type: vk::StructureType::COPY_BUFFER_TO_IMAGE_INFO_2,
                                p_next: ptr::null(),
                                src_buffer: unsafe { *src_buffer.as_raw() },
                                dst_image: unsafe { *dst_image.as_raw() },
                                dst_image_layout: vk::ImageLayout::UNDEFINED,
                                region_count: 1,
                                p_regions: &vk::BufferImageCopy2 {
                                    s_type: vk::StructureType::BUFFER_IMAGE_COPY_2,
                                    p_next: ptr::null(),
                                    buffer_offset: *src_offset,
                                    buffer_row_length: 0,
                                    buffer_image_height: 0,
                                    image_subresource: vk::ImageSubresourceLayers {
                                        aspect_mask: vk::ImageAspectFlags::COLOR,
                                        mip_level: 0,
                                        base_array_layer: 0,
                                        layer_count: 1,
                                    },
                                    image_offset: *dst_offset,
                                    image_extent: *extent,
                                    _marker: Default::default(),
                                },
                                _marker: Default::default(),
                            },
                        );
                    }
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
                    .unwrap();
            }
        }
        fence.fence_await().await?;
        processor
            .request
            .callback
            .send(match processor.request.request {
                TransferRequest::Buffer {
                    src_buffer,
                    dst_buffer,
                    ..
                } => Ok(TransferRequestCallback::Buffer {
                    src_buffer,
                    dst_buffer,
                }),
                TransferRequest::Image {
                    src_buffer,
                    dst_image,
                    ..
                } => Ok(TransferRequestCallback::Image {
                    src_buffer,
                    dst_image,
                }),
            })
            .unwrap();
        drop(permits);
        Ok(())
    }

    pub fn get_device(&self) -> dagal::device::LogicalDevice {
        self.device.clone()
    }
}
