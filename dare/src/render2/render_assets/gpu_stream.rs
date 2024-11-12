use crate::prelude as dare;
use crate::render2::prelude::util::TransferRequestCallback;
use async_stream::{stream, try_stream};
use dagal::allocators::Allocator;
use dagal::ash::vk;
use dagal::traits::AsRaw;
use futures::StreamExt;
use futures_core::Stream;

pub fn gpu_buffer_stream<'a, T, A>(
    mut staging_buffer: dagal::resource::Buffer<A>,
    dst_buffer: dagal::resource::Buffer<A>,
    transfer_pool: dare::render::util::TransferPool<A>,
    stream: impl Stream<Item = anyhow::Result<T>> + 'a + Send,
) -> impl Stream<
    Item = anyhow::Result<Option<(dagal::resource::Buffer<A>, dagal::resource::Buffer<A>)>>,
>
       + 'a
       + Send
where
    T: AsRef<[u8]> + Send + 'a,
    A: Allocator + 'static,
{
    stream! {
        let mut initial_progress = 0;
        let mut staging_buffer = Some(staging_buffer);
        let mut dest_buffer = Some(dst_buffer);

        futures::pin_mut!(stream);
        loop {
            if let Some(data) = stream.next().await {
                let data = data?;
                let data_ref = data.as_ref();
                let length = data_ref.len() as vk::DeviceSize;
                // write to staging
                staging_buffer.as_mut().unwrap().write(0, data_ref)?;
                let transfer_future = transfer_pool.transfer_gpu(
                    dare::render::util::TransferRequest::Buffer {
                            src_buffer: staging_buffer.take().unwrap(),
                            dst_buffer: dest_buffer.take().unwrap(),
                            src_offset: 0,
                            dst_offset: initial_progress,
                            length,
                    },
                );

                let res = transfer_future.await?;
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

                yield Ok(None);
            } else if staging_buffer.is_some() && dest_buffer.is_some() {
                yield Ok(Some((staging_buffer.take().unwrap(), dest_buffer.take().unwrap())));
            }
        }
    }
}
