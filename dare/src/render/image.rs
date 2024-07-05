use std::ptr;

use anyhow::Result;

use dagal::allocators::Allocator;
use dagal::ash::vk;
use dagal::command::command_buffer::CmdBuffer;
use dagal::resource;
use dagal::resource::traits::Resource;

pub async fn generate_mip_maps<A: Allocator>(device: dagal::device::LogicalDevice, queue: dagal::device::Queue, image: resource::Image<A>) -> Result<resource::Image<A>> {
    let mip_levels = image.mip_levels();
    let mut extent = vk::Extent2D {
        width: image.extent().width,
        height: image.extent().height,
    };
    if mip_levels <= 1 {
        return Ok(image);
    }
    let fence = dagal::sync::Fence::new(device.clone(), vk::FenceCreateFlags::empty())?;
    let vk_queue = queue.acquire_queue_lock()?;
    let command_pool = dagal::command::CommandPool::new(
        device.clone(),
        &queue,
        vk::CommandPoolCreateFlags::empty()
    )?;
    let cmd_buffer = command_pool.allocate(1)?.pop().unwrap();
    let cmd_buffer = cmd_buffer.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT).unwrap();
    image.transition(&cmd_buffer, &queue, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
    for mip_level in 0..mip_levels {
        let half_size = vk::Extent2D {
            width: extent.width / 2,
            height: extent.height / 2,
        };
        unsafe {
            device.get_handle()
                  .cmd_pipeline_barrier2(
                      cmd_buffer.handle(),
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
                              dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
                              dst_access_mask: vk::AccessFlags2::MEMORY_WRITE | vk::AccessFlags2::MEMORY_READ,
                              old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                              new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                              src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                              dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                              image: image.handle(),
                              subresource_range: vk::ImageSubresourceRange {
                                  aspect_mask: vk::ImageAspectFlags::COLOR,
                                  base_mip_level: mip_level,
                                  level_count: 1,
                                  base_array_layer: 0,
                                  layer_count: 1,
                              },
                              _marker: Default::default(),
                          },
                          _marker: Default::default(),
                      })
        }
        if mip_level < mip_levels - 1 {
            let blit_region = vk::ImageBlit2 {
                s_type: vk::StructureType::IMAGE_BLIT_2,
                p_next: ptr::null(),
                src_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                src_offsets: [
                    vk::Offset3D::default(),
                    vk::Offset3D {
                        x: extent.width as i32,
                        y: extent.height as i32,
                        z: 1,
                    }
                ],
                dst_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: mip_level + 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                dst_offsets: [
                    vk::Offset3D::default(),
                    vk::Offset3D {
                        x: half_size.width as i32,
                        y: half_size.height as i32,
                        z: 1,
                    }
                ],
                _marker: Default::default(),
            };
            unsafe {
                device.get_handle()
                      .cmd_blit_image2(
                          cmd_buffer.handle(),
                          &vk::BlitImageInfo2 {
                              s_type: vk::StructureType::BLIT_IMAGE_INFO_2,
                              p_next: ptr::null(),
                              src_image: image.handle(),
                              src_image_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                              dst_image: image.handle(),
                              dst_image_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                              region_count: 1,
                              p_regions: &blit_region,
                              filter: vk::Filter::LINEAR,
                              _marker: Default::default(),
                          }
                      )
            }
            extent = half_size;
        }
    }
    image.transition(&cmd_buffer, &queue, vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::GENERAL);
    let cmd_buffer = cmd_buffer.end()?;
    unsafe {
        command_pool.get_device().get_handle().queue_submit2(
            *vk_queue,
            &[vk::SubmitInfo2 {
                s_type: vk::StructureType::SUBMIT_INFO_2,
                p_next: ptr::null(),
                flags: vk::SubmitFlags::empty(),
                wait_semaphore_info_count: 0,
                p_wait_semaphore_infos: ptr::null(),
                command_buffer_info_count: 1,
                p_command_buffer_infos: &cmd_buffer.submit_info(),
                signal_semaphore_info_count: 0,
                p_signal_semaphore_infos: ptr::null(),
                _marker: Default::default(),
            }],
            fence.handle()
        )?
    }
    drop(vk_queue);
    fence.await?;
    Ok(image)
}