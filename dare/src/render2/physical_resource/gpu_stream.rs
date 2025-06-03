use crate::prelude as dare;
use dagal::allocators::Allocator;
use dagal::ash::vk;
use dagal::util::format::get_size_from_vk_format;
use futures::StreamExt;
use futures_core::Stream;
use futures_core::stream::BoxStream;

/// Streams data from a source stream to a GPU buffer
///
/// Returns back the staging buffer and the destination buffer as a tuple
pub fn gpu_buffer_stream<'a, T, A>(
    staging_buffer: dagal::resource::Buffer<A>,
    dst_buffer: dagal::resource::Buffer<A>,
    transfer_pool: dare::render::util::TransferPool<A>,
    source_stream: impl Stream<Item = anyhow::Result<T>> + 'a + Send,
) -> impl Stream<Item = Option<(dagal::resource::Buffer<A>, dagal::resource::Buffer<A>)>> + 'a + Send
where
    T: AsRef<[u8]> + Send + 'a,
    A: Allocator + 'static,
{
    assert!(staging_buffer.get_size() <= transfer_pool.gpu_staging_size());

    // filter out source information (we just panic for now)
    let filtered_stream: BoxStream<'a, T> = source_stream
        .filter_map(|res| async move {
            match res {
                Ok(n) => Some(n),
                Err(e) => {
                    panic!("Error found in GPU stream: {e}");
                    None
                }
            }
        })
        .boxed();

    // framer
    // define const
    let hardware_partition_size: usize = transfer_pool
        .gpu_staging_size()
        .min(transfer_pool.cpu_staging_size()) as usize;
    let framer =
        dare::asset2::loaders::framer::Framer::new(filtered_stream, hardware_partition_size);

    // create gpu stream + state and unfold (folding, but instead of a stream -> single, single -> stream)
    let state = (
        Some(staging_buffer), // staging buffer
        Some(dst_buffer),     // dst buffer
        transfer_pool,        // transfer pool to send transfer request
        framer,               // stream
        0 as vk::DeviceSize,  // progress made
        false,                // done tracking
    );
    futures::stream::unfold(
        state,
        move |(
            mut staging_opt,
            mut dest_opt,
            transfer_pool,
            mut framer,
            mut progress,
            mut done,
        )| async move {
            if let Some(chunk) = framer.next().await {
                let chunk_size = chunk.len();
                assert!(chunk_size <= hardware_partition_size);

                // write partition into staging
                staging_opt.as_mut().map(|staging| staging.write(0, &chunk));
                let (src_buffer, dst_buffer) = transfer_pool
                    .buffer_to_buffer_transfer(dare::render::util::TransferBufferToBuffer {
                        src_buffer: staging_opt.take().unwrap(),
                        dst_buffer: dest_opt.take().unwrap(),
                        src_offset: 0,
                        dst_offset: progress,
                        length: chunk_size as vk::DeviceSize,
                    })
                    .await
                    .unwrap();

                // unpacked returned buffers
                staging_opt = Some(src_buffer);
                dest_opt = Some(dst_buffer);
                progress += chunk_size as vk::DeviceSize;

                Some((
                    None,
                    (staging_opt, dest_opt, transfer_pool, framer, progress, done),
                ))
            } else if !done {
                done = true;
                if let (Some(s), Some(d)) = (staging_opt.take(), dest_opt.take()) {
                    Some((
                        Some((s, d)),
                        (staging_opt, dest_opt, transfer_pool, framer, progress, done),
                    ))
                } else {
                    // nothing to yield
                    None
                }
            } else {
                // done!
                None
            }
        },
    )
}

/// Streams data from a source stream to a GPU texture
///
/// Returns back the staging buffer and the destination image as a tuple
pub fn gpu_texture_stream<'a, T, A>(
    staging_buffer: dagal::resource::Buffer<A>,
    dst_image: dagal::resource::Image<A>,
    src_image_layout: vk::ImageLayout,
    dst_image_layout: Option<vk::ImageLayout>,
    transfer_pool: dare::render::util::TransferPool<A>,
    source_stream: impl Stream<Item = anyhow::Result<T>> + 'a + Send,
) -> impl Stream<Item = Option<(dagal::resource::Buffer<A>, dagal::resource::Image<A>)>> + 'a + Send
where
    T: AsRef<[u8]> + Send + 'a,
    A: Allocator + 'static,
{
    // precompute sizes
    let px_size = get_size_from_vk_format(&dst_image.format()) as usize;
    let width = dst_image.extent().width as usize;
    let height = dst_image.extent().height as usize;
    let depth = dst_image.extent().depth as usize;
    let row_bytes = width * px_size;

    // framer
    let max_stage = transfer_pool
        .gpu_staging_size()
        .min(transfer_pool.cpu_staging_size()) as usize;
    let max_stage_bytes = (max_stage / px_size) * px_size;

    let filtered = source_stream.filter_map(|r| async move { r.ok() }).boxed();
    let framer = dare::asset2::loaders::framer::Framer::new(filtered, max_stage_bytes);

    let init = (
        framer,
        Some(staging_buffer),
        Some(dst_image),
        transfer_pool,
        0usize, // x
        0usize, // y
        0usize, // z
        Vec::<u8>::new(),
        false, // done
    );

    futures::stream::unfold(
        init,
        move |(
            mut framer,
            mut staging_opt,
            mut dest_opt,
            transfer_pool,
            mut x,
            mut y,
            mut z,
            mut acc,
            mut done,
        )| async move {
            // pull next bytes if not EOF
            if !done {
                if let Some(chunk) = framer.next().await {
                    acc.extend_from_slice(chunk.as_ref());
                } else {
                    done = true;
                }
            }

            // while we have any pixels buffered, flush them
            while !acc.is_empty() && !staging_opt.is_none() {
                let avail_pixels = acc.len() / px_size;

                // are we at the start of a row?
                if x == 0 && avail_pixels >= width {
                    // full-row batching
                    let full_rows = avail_pixels / width;
                    let max_rows_at_once = max_stage_bytes / row_bytes;
                    let rows_to_send = full_rows.min(max_rows_at_once).max(1);
                    let send_bytes = rows_to_send * row_bytes;

                    let mut buf = staging_opt.take().unwrap();
                    buf.write(0, &acc[..send_bytes]).unwrap();

                    let (buf, img) = transfer_pool
                        .buffer_to_image_transfer(dare::render::util::TransferBufferToImage {
                            src_buffer: buf,
                            dst_image: dest_opt.take().unwrap(),
                            src_offset: 0,
                            dst_offset: vk::Offset3D {
                                x: 0,
                                y: y as i32,
                                z: z as i32,
                            },
                            extent: vk::Extent3D {
                                width: width as u32,
                                height: rows_to_send as u32,
                                depth: 1,
                            },
                            src_layout: src_image_layout,
                            dst_layout: dst_image_layout,
                        })
                        .await
                        .unwrap();

                    // consume and update state
                    acc.drain(..send_bytes);
                    staging_opt = Some(buf);
                    dest_opt = Some(img);
                    y += rows_to_send;
                    while y >= height {
                        y -= height;
                        z += 1;
                        if z >= depth {
                            done = true;
                            break;
                        }
                    }
                    // x remains 0 after full-row batch
                } else {
                    // partial-row or mid-row flush
                    let row_rem = width - x; // pixels left in this row
                    let pixels_to_send = avail_pixels.min(row_rem).max(1);
                    let send_bytes = pixels_to_send * px_size;

                    let mut buf = staging_opt.take().unwrap();
                    buf.write(0, &acc[..send_bytes]).unwrap();

                    let (buf, img) = transfer_pool
                        .buffer_to_image_transfer(dare::render::util::TransferBufferToImage {
                            src_buffer: buf,
                            dst_image: dest_opt.take().unwrap(),
                            src_offset: 0,
                            dst_offset: vk::Offset3D {
                                x: x as i32,
                                y: y as i32,
                                z: z as i32,
                            },
                            extent: vk::Extent3D {
                                width: pixels_to_send as u32,
                                height: 1,
                                depth: 1,
                            },
                            src_layout: src_image_layout,
                            dst_layout: dst_image_layout,
                        })
                        .await
                        .unwrap();

                    acc.drain(..send_bytes);
                    staging_opt = Some(buf);
                    dest_opt = Some(img);

                    // advance x/y/z
                    x += pixels_to_send;
                    if x >= width {
                        x = 0;
                        y += 1;
                        if y >= height {
                            y = 0;
                            z += 1;
                            if z >= depth {
                                done = true;
                            }
                        }
                    }
                }

                if done {
                    break;
                }
            }

            // Once done, emit the final (buf, img)
            if done {
                if let (Some(b), Some(i)) = (staging_opt.take(), dest_opt.take()) {
                    return Some((
                        Some((b, i)),
                        (
                            framer,
                            staging_opt,
                            dest_opt,
                            transfer_pool,
                            x,
                            y,
                            z,
                            acc,
                            done,
                        ),
                    ));
                }
                return None;
            }

            // Otherwise keep streaming
            Some((
                None,
                (
                    framer,
                    staging_opt,
                    dest_opt,
                    transfer_pool,
                    x,
                    y,
                    z,
                    acc,
                    done,
                ),
            ))
        },
    )
}
