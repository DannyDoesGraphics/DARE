use crate::prelude as dare;
use async_stream::stream;
use dagal::allocators::Allocator;
use dagal::ash::vk;
use futures::StreamExt;
use futures_core::Stream;
use futures_core::stream::BoxStream;

/// Streams data from a source stream to a GPU buffer
///
/// Returns back the staging buffer and the destination buffer as a tuple
pub fn gpu_buffer_stream<'a, T, A>(
    mut staging_buffer: dagal::resource::Buffer<A>,
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
    let mut framer =
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
                staging_opt
                    .as_mut()
                    .map(|mut staging| staging.write(0, &chunk));
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
    mut staging_buffer: dagal::resource::Buffer<A>,
    dst_image: dagal::resource::Image<A>,
    transfer_pool: dare::render::util::TransferPool<A>,
    stream: impl Stream<Item = anyhow::Result<T>> + 'a + Send,
) -> ()
where
    T: AsRef<[u8]> + Send + 'a,
    A: Allocator + 'static,
{
    assert!(staging_buffer.get_size() <= transfer_pool.gpu_staging_size());
    todo!()
}
