use crate::prelude as dare;
use crate::render2::prelude::util::TransferRequestCallback;
use async_stream::stream;
use dagal::allocators::Allocator;
use dagal::ash::vk;
use futures::{StreamExt, TryStreamExt};
use futures_core::Stream;

pub fn gpu_buffer_stream<'a, T, A>(
    mut staging_buffer: dagal::resource::Buffer<A>,
    dst_buffer: dagal::resource::Buffer<A>,
    transfer_pool: dare::render::util::TransferPool<A>,
    stream: impl Stream<Item = anyhow::Result<T>> + 'a + Send,
) -> impl Stream<
    Item = Option<(dagal::resource::Buffer<A>, dagal::resource::Buffer<A>)>,
>
       + 'a
       + Send
where
    T: AsRef<[u8]> + Send + 'a,
    A: Allocator + 'static,
{
    assert!(staging_buffer.get_size() <= transfer_pool.gpu_staging_size() );
    stream! {
        let mut initial_progress = 0;
        let mut staging_buffer = Some(staging_buffer);
        let mut dest_buffer = Some(dst_buffer);

        // stabilize the stream to within buffer stream restrictions
        let stream = stream.filter_map(|item| async move {
            match item {
            Ok(value) => Some(value), // Pass the value through
            Err(err) => {
                panic!("Stream error: {}", err); // Log or handle the error
                None // Drop the error
            }
            }
        }).boxed();
        let mut stream = dare::asset2::loaders::framer::Framer::new(stream, staging_buffer.as_ref().unwrap().get_size() as usize).boxed();
        loop {
            if let Some(data) = stream.next().await {
                assert!(data.len() <= transfer_pool.gpu_staging_size() as usize);
                let length = data.len() as vk::DeviceSize;
                // write to staging
                staging_buffer.as_mut().unwrap().write(0, &data).unwrap();
                let transfer_future = transfer_pool.transfer_gpu(
                    dare::render::util::TransferRequest::Buffer {
                            src_buffer: staging_buffer.take().unwrap(),
                            dst_buffer: dest_buffer.take().unwrap(),
                            src_offset: 0,
                            dst_offset: initial_progress,
                            length,
                    },
                );
                let res = transfer_future.await.unwrap();
                match res {
                    TransferRequestCallback::Buffer{
                        dst_buffer, src_buffer, ..
                    } => {
                        dest_buffer = Some(dst_buffer);
                        staging_buffer = Some(src_buffer);
                    },
                    _ => panic!()
                }

                initial_progress += length;

                yield None;
            } else if staging_buffer.is_some() && dest_buffer.is_some() {
                yield Some((staging_buffer.take().unwrap(), dest_buffer.take().unwrap()));
            }
        }
    }
}
