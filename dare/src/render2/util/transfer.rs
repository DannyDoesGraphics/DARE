use anyhow::Result;
use dagal::allocators::{Allocator, ArcAllocator};
use dagal::ash::vk;
use dagal::ash::vk::Queue;
use dagal::command::command_buffer::CmdBuffer;
use dagal::resource;
use dagal::resource::traits::Resource;
use dagal::traits::AsRaw;
use std::ptr;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Copy, Clone)]
pub struct BufferTransferRequest {
    pub src_buffer: vk::Buffer,
    pub dst_buffer: vk::Buffer,
    pub src_offset: vk::DeviceSize,
    pub dst_offset: vk::DeviceSize,
    pub length: vk::DeviceSize,
}

#[derive(Debug, Copy, Clone)]
pub struct ImageTransferRequest {
    pub src_buffer: vk::Buffer,
    pub src_offset: vk::DeviceSize,
    pub src_length: vk::DeviceSize,
    pub extent: vk::Extent3D,
    pub dst_image: vk::Image,
    pub dst_offset: vk::Offset3D,
    pub dst_length: vk::DeviceSize,
}

#[derive(Debug, Copy, Clone)]
pub enum TransferRequest {
    Buffer(BufferTransferRequest),
    Image(ImageTransferRequest),
}

pub struct TransferRequestInner {
    request: TransferRequest,
    callback: tokio::sync::oneshot::Sender<()>,
}

/// Allows for quick transfers
#[derive(Debug, Clone)]
pub struct TransferPool {
    device: dagal::device::LogicalDevice,
    semaphore: Arc<tokio::sync::Semaphore>,
    sender: Arc<tokio::sync::mpsc::Sender<TransferRequestInner>>,
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

impl TransferPool {
    pub fn new<A: Allocator + 'static>(
        device: dagal::device::LogicalDevice,
        size: vk::DeviceSize,
        allocator: ArcAllocator<A>,
        command_pool: Arc<dagal::command::CommandPool>,
        queues: Arc<Vec<dagal::device::Queue>>,
    ) -> Result<Self> {
        let (sender, receiver) = tokio::sync::mpsc::channel::<TransferRequestInner>(32);
        let sf = Self {
            device: device.clone(),
            semaphore: Arc::new(tokio::sync::Semaphore::new(size as usize)),
            sender: Arc::new(sender),
        };
        let semaphore = sf.semaphore.clone();
        tokio::task::spawn(async move {
            Self::process_upload_requests(
                semaphore,
                receiver,
                device,
                allocator,
                command_pool,
                queues,
            )
                .await?;
            Ok::<(), anyhow::Error>(())
        });
        Ok(sf)
    }

    /// Submit a transfer request to be transferred onto the gpu
    pub async fn transfer_gpu(&self, request: TransferRequest) -> Result<()> {
        let (sender, reciever) = tokio::sync::oneshot::channel::<()>();
        self.sender
            .send(TransferRequestInner {
                request,
                callback: sender,
            })
            .await?;
        Ok(reciever.await?)
    }

    async fn process_upload_requests<A: Allocator>(
        semaphore: Arc<tokio::sync::Semaphore>,
        mut receiver: tokio::sync::mpsc::Receiver<TransferRequestInner>,
        device: dagal::device::LogicalDevice,
        mut allocator: ArcAllocator<A>,
        command_pool: Arc<dagal::command::CommandPool>,
        queues: Arc<Vec<dagal::device::Queue>>,
    ) -> Result<()> {
        while let Some(request) = receiver.recv().await {
            let permits = semaphore
                .acquire_many(match &request.request {
                    TransferRequest::Buffer(req) => req.length,
                    TransferRequest::Image(req) => req.dst_length,
                } as u32)
                .await?;
            let (index, queue_guard) = pick_available_queues(queues.as_slice()).await;
            let queue = queues.get(index).unwrap().clone();
            let fence = dagal::sync::Fence::new(device.clone(), vk::FenceCreateFlags::empty())?;
            let command_buffer = command_pool
                .allocate(1)?
                .pop()
                .unwrap()
                .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
                .unwrap();
            let buffer = resource::Buffer::new(resource::BufferCreateInfo::NewEmptyBuffer {
                device: device.clone(),
                allocator: &mut allocator,
                size: match &request.request {
                    TransferRequest::Buffer(req) => req.length,
                    TransferRequest::Image(req) => req.dst_length,
                },
                memory_type: dagal::allocators::MemoryLocation::GpuOnly,
                usage_flags: vk::BufferUsageFlags::TRANSFER_SRC
                    | vk::BufferUsageFlags::TRANSFER_DST,
            })?;

            {
                unsafe {
                    device.get_handle().cmd_copy_buffer2(
                        command_buffer.handle(),
                        &vk::CopyBufferInfo2 {
                            s_type: vk::StructureType::COPY_BUFFER_INFO_2,
                            p_next: ptr::null(),
                            src_buffer: match &request.request {
                                TransferRequest::Buffer(req) => req.src_buffer,
                                TransferRequest::Image(req) => req.src_buffer,
                            },
                            dst_buffer: *buffer.as_raw(),
                            region_count: 1,
                            p_regions: &vk::BufferCopy2 {
                                s_type: vk::StructureType::BUFFER_COPY_2,
                                p_next: ptr::null(),
                                src_offset: match &request.request {
                                    TransferRequest::Buffer(req) => req.src_offset,
                                    TransferRequest::Image(req) => req.src_offset,
                                },
                                dst_offset: 0,
                                size: match &request.request {
                                    TransferRequest::Buffer(req) => req.length,
                                    TransferRequest::Image(req) => req.src_length,
                                },
                                _marker: Default::default(),
                            },
                            _marker: Default::default(),
                        },
                    );
                    device.get_handle().cmd_pipeline_barrier2(
                        command_buffer.handle(),
                        &vk::DependencyInfo {
                            s_type: vk::StructureType::DEPENDENCY_INFO,
                            p_next: ptr::null(),
                            dependency_flags: vk::DependencyFlags::empty(),
                            memory_barrier_count: 0,
                            p_memory_barriers: ptr::null(),
                            buffer_memory_barrier_count: 1,
                            p_buffer_memory_barriers: &vk::BufferMemoryBarrier2 {
                                s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
                                p_next: ptr::null(),
                                src_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                                src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
                                dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                                dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
                                src_queue_family_index: queue.get_family_index(),
                                dst_queue_family_index: queue.get_family_index(),
                                buffer: *buffer.as_raw(),
                                offset: 0,
                                size: match &request.request {
                                    TransferRequest::Buffer(req) => req.length,
                                    TransferRequest::Image(req) => req.src_length,
                                },
                                _marker: Default::default(),
                            },
                            image_memory_barrier_count: 0,
                            p_image_memory_barriers: ptr::null(),
                            _marker: Default::default(),
                        },
                    );
                    // Handle images and buffer unique transfers
                    match &request.request {
                        TransferRequest::Buffer(request) => {
                            device.get_handle().cmd_copy_buffer2(
                                command_buffer.handle(),
                                &vk::CopyBufferInfo2 {
                                    s_type: vk::StructureType::COPY_BUFFER_INFO_2,
                                    p_next: ptr::null(),
                                    src_buffer: request.src_buffer,
                                    dst_buffer: *buffer.as_raw(),
                                    region_count: 1,
                                    p_regions: &vk::BufferCopy2 {
                                        s_type: vk::StructureType::BUFFER_COPY_2,
                                        p_next: ptr::null(),
                                        src_offset: 0,
                                        dst_offset: request.dst_offset,
                                        size: request.length,
                                        _marker: Default::default(),
                                    },
                                    _marker: Default::default(),
                                },
                            );
                        }
                        TransferRequest::Image(image) => {
                            device.get_handle().cmd_copy_buffer_to_image2(
                                command_buffer.handle(),
                                &vk::CopyBufferToImageInfo2 {
                                    s_type: vk::StructureType::COPY_BUFFER_TO_IMAGE_INFO_2,
                                    p_next: ptr::null(),
                                    src_buffer: *buffer.as_raw(),
                                    dst_image: image.dst_image,
                                    dst_image_layout: vk::ImageLayout::UNDEFINED,
                                    region_count: 1,
                                    p_regions: &vk::BufferImageCopy2 {
                                        s_type: vk::StructureType::BUFFER_IMAGE_COPY_2,
                                        p_next: ptr::null(),
                                        buffer_offset: image.src_offset,
                                        buffer_row_length: 0,
                                        buffer_image_height: 0,
                                        image_subresource: vk::ImageSubresourceLayers {
                                            aspect_mask: vk::ImageAspectFlags::COLOR,
                                            mip_level: 0,
                                            base_array_layer: 0,
                                            layer_count: 1,
                                        },
                                        image_offset: image.dst_offset,
                                        image_extent: image.extent,
                                        _marker: Default::default(),
                                    },
                                    _marker: Default::default(),
                                },
                            );
                        }
                    }
                }
                let command_buffer = command_buffer.end()?;
                {
                    command_buffer
                        .submit(
                            *queue_guard,
                            &[vk::SubmitInfo2 {
                                s_type: vk::StructureType::SUBMIT_INFO,
                                p_next: ptr::null(),
                                flags: vk::SubmitFlags::empty(),
                                wait_semaphore_info_count: 0,
                                p_wait_semaphore_infos: ptr::null(),
                                command_buffer_info_count: 0,
                                p_command_buffer_infos: ptr::null(),
                                signal_semaphore_info_count: 0,
                                p_signal_semaphore_infos: ptr::null(),
                                _marker: Default::default(),
                            }],
                            fence.handle(),
                        )
                        .unwrap();
                }
            }
            fence.await?;
            request.callback.send(()).unwrap();
        }
        Ok(())
    }
}
