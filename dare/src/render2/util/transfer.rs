use crate::util::either::Either;
use anyhow::Result;
use dagal::allocators::{Allocator, ArcAllocator};
use dagal::ash::vk;
use dagal::ash::vk::{Handle, Queue};
use dagal::command::command_buffer::CmdBuffer;
use dagal::resource;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use futures::stream::FuturesUnordered;
use futures::{FutureExt, TryFutureExt};
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
pub enum TransferRequestRaw {
    Buffer {
        src_buffer: vk::Buffer,
        dst_buffer: vk::Buffer,
        src_offset: vk::DeviceSize,
        dst_offset: vk::DeviceSize,
        length: vk::DeviceSize,
    },
    Image {
        src_buffer: vk::Buffer,
        src_offset: vk::DeviceSize,
        src_length: vk::DeviceSize,
        extent: vk::Extent3D,
        dst_image: vk::Image,
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

struct TransferRequestInnerSafe<A: Allocator> {
    request: TransferRequest<A>,
    callback: tokio::sync::oneshot::Sender<Result<TransferRequestCallback<A>>>,
}

struct TransferRequestInnerRaw {
    request: TransferRequestRaw,
    callback: tokio::sync::oneshot::Sender<Result<()>>,
}

enum TransferRequestInner<A: Allocator> {
    TransferRequest(TransferRequestInnerSafe<A>),
    TransferRequestRaw(TransferRequestInnerRaw),
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
            .send(TransferRequestInner::TransferRequest(
                TransferRequestInnerSafe {
                    request,
                    callback: sender,
                },
            ))?;
        receiver.await?
    }

    /// Submit a transfer request to be transferred onto the gpu
    pub async unsafe fn transfer_gpu_raw(&self, request: TransferRequestRaw) -> Result<()> {
        let (sender, receiver) = tokio::sync::oneshot::channel::<Result<()>>();
        self.inner
            .sender
            .send(TransferRequestInner::TransferRequestRaw(
                TransferRequestInnerRaw {
                    request,
                    callback: sender,
                },
            ))?;
        receiver.await?
    }

    async fn process_upload_requests(
        semaphore: Arc<tokio::sync::Semaphore>,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<TransferRequestInner<A>>,
        device: dagal::device::LogicalDevice,
        queues: Arc<[dagal::device::Queue]>,
        mut shut_recv: Arc<tokio::sync::Notify>,
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
        let mut tasks: FuturesUnordered<tokio::task::JoinHandle<Result<()>>> =
            FuturesUnordered::new();

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
                    let dst_length: u64 = match &request {
                        TransferRequestInner::TransferRequest(request) => match &request.request {
                            TransferRequest::Buffer {
                                length,
                                ..
                            } => *length as u64,
                            TransferRequest::Image {
                                src_length,
                                ..
                            } => *src_length as u64,
                        },
                        TransferRequestInner::TransferRequestRaw(request) => match &request.request {
                            TransferRequestRaw::Buffer { length, .. } => *length,
                            TransferRequestRaw::Image { src_length, .. } => *src_length,
                        }
                    };
                    if dst_length > gpu_staging_size as u64 {
                        tracing::error!("Exceeds {dst_length} > {gpu_staging_size}");
                        match request {
                            TransferRequestInner::TransferRequest(request) => request.callback.send(Err(anyhow::anyhow!("Size exceeds GPU staging size"))).unwrap(),
                            TransferRequestInner::TransferRequestRaw(request) => request.callback.send(Err(anyhow::anyhow!("Size exceeds GPU staging size"))).unwrap(),
                        }
                        continue;
                    }

                    // validate request is sane
                    if match &request {
                        TransferRequestInner::TransferRequest(request) => match &request.request {
                            TransferRequest::Buffer {
                                src_buffer,
                                dst_buffer,
                                src_offset,
                                dst_offset,
                                length,
                            } => dst_buffer.get_size() < *dst_offset + *length,
                            _ => false,
                        }
                        TransferRequestInner::TransferRequestRaw(request) => true,
                    } {
                        tracing::error!("Cannot transfer due to malformed request");
                        match request {
                            TransferRequestInner::TransferRequest(request) => request.callback.send(Err(anyhow::anyhow!("Malformed request"))).unwrap(),
                            TransferRequestInner::TransferRequestRaw(request) => request.callback.send(Err(anyhow::anyhow!("Malformed request"))).unwrap(),
                        }
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
                    match request {
                        TransferRequestInner::TransferRequest(request) => {
                            let task = tokio::spawn(Self::process_single_transfer(processor, request));
                            tasks.push(task);
                        }
                        TransferRequestInner::TransferRequestRaw(request) => unsafe {
                            let callback = request.callback;
                            let task = tokio::spawn(async move {
                                let r = Self::process_single_transfer_raw(processor, request.request).await;
                                callback.send(match r {
                                    Ok(_) => anyhow::Ok(()),
                                    Err(_) => Err(anyhow::anyhow!("Failed raw transfer")),
                                }).unwrap();
                                anyhow::Ok(())
                            });
                            tasks.push(task);
                        }
                    }
                }
            }
        }
    }

    async unsafe fn process_single_transfer_raw(
        processor: TransferProcessor,
        request: TransferRequestRaw,
    ) -> Result<()> {
        let src_length = match &request {
            TransferRequestRaw::Buffer { length, .. } => *length,
            TransferRequestRaw::Image { src_length, .. } => *src_length,
        } as u32;
        // Acquire necessary semaphore permits and select an available queue
        let permits = processor.semaphore.acquire_many(src_length).await?;
        let (index, queue_guard) = pick_available_queues(&processor.queues).await;
        let fence: &dagal::sync::Fence = &processor.fences[index];
        // wait for fence to be cleared
        fence.fence_await().await?;
        fence.reset().unwrap();
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
                    match &request {
                        TransferRequestRaw::Buffer {
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
                                    src_buffer: *src_buffer,
                                    dst_buffer: *dst_buffer,
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
                        }
                        TransferRequestRaw::Image {
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
                                    src_buffer: *src_buffer,
                                    dst_image: *dst_image,
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
                        .map_err(|e| {
                            tracing::error!("Failed to submit transfer command: {:?}", e);
                            e
                        })
                        .unwrap();
                }
            }
            fence.fence_await().await?;
            drop(queue_guard);
            let num_permits = permits.num_permits();
            processor.semaphore.add_permits(num_permits);
            drop(permits);
            anyhow::Ok(())
        };
        match res {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Failed to complete transfer: {:?}", e);
                fence.reset().unwrap();
            }
        }
        Ok(())
    }

    async fn process_single_transfer(
        processor: TransferProcessor,
        request: TransferRequestInnerSafe<A>,
    ) -> Result<()> {
        unsafe {
            Self::process_single_transfer_raw(
                processor,
                match &request.request {
                    TransferRequest::Buffer {
                        src_buffer,
                        dst_buffer,
                        src_offset,
                        dst_offset,
                        length,
                    } => TransferRequestRaw::Buffer {
                        src_buffer: *src_buffer.as_raw(),
                        dst_buffer: *dst_buffer.as_raw(),
                        src_offset: *src_offset,
                        dst_offset: *dst_offset,
                        length: *length,
                    },
                    TransferRequest::Image {
                        src_buffer,
                        src_offset,
                        src_length,
                        extent,
                        dst_image,
                        dst_offset,
                        dst_length,
                    } => TransferRequestRaw::Image {
                        src_buffer: *src_buffer.as_raw(),
                        src_offset: *src_offset,
                        src_length: *src_length,
                        extent: *extent,
                        dst_image: *dst_image.as_raw(),
                        dst_offset: *dst_offset,
                        dst_length: *dst_length,
                    },
                },
            )
        }
        .await?;
        request
            .callback
            .send(match request.request {
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
            .map_err(|e| {
                tracing::error!("Failed to send transfer callback: {:?}", e);
                e
            })
            .unwrap();
        Ok(())
    }

    pub fn get_device(&self) -> dagal::device::LogicalDevice {
        self.device.clone()
    }
}
