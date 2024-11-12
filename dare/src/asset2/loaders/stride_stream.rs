use bevy_tasks::futures_lite::StreamExt;
use derivative::Derivative;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Builds a stride stream
pub struct StrideStreamBuilder {
    /// Offset ib bytes
    pub offset: usize,
    /// Size of elements in bytes
    pub element_size: usize,
    /// Stride of elements in bytes
    pub element_stride: usize,
    /// \# of elements in bytes
    pub element_count: usize,
    /// Maximum frame size of each yield in bytes
    pub frame_size: usize,
}

impl StrideStreamBuilder {
    pub fn build<T: AsRef<[u8]>>(
        self,
        stream: futures_core::stream::LocalBoxStream<anyhow::Result<T>>,
    ) -> StrideStream<T> {
        StrideStream {
            offset: self.offset,
            element_size: self.element_size,
            element_stride: self.element_stride,
            element_count: self.element_count,
            frame_size: self.frame_size,
            data_stream: stream,
            element_processed: 0,
            bytes_recv: 0,
            processed: Vec::with_capacity(self.frame_size),
            buffer: Vec::new(),
        }
    }
}

/// StrideStream is a post process stream that collects all data from an incoming [`futures_core::stream::LocalBoxStream`]
///
/// # Cancellation safety
/// As long as StrideStream is kept alive outside the cancellation scope,
/// it is cancellation safe as it keeps state
#[derive(Derivative)]
#[derivative(Debug)]
pub struct StrideStream<'a, T: AsRef<[u8]>> {
    offset: usize,
    element_size: usize,
    element_stride: usize,
    element_count: usize,
    frame_size: usize,

    #[derivative(Debug = "ignore")]
    /// We do not expect data_stream to be processed on another thread
    data_stream: futures_core::stream::LocalBoxStream<'a, anyhow::Result<T>>,

    /// \# of elements processed
    element_processed: usize,
    /// \# of bytes received from the stream (for offset)
    bytes_recv: usize,
    /// Bytes of the processed elements
    /// waiting to the yielded out
    processed: Vec<u8>,
    /// Bytes of elements awaiting processing
    buffer: Vec<u8>,
}
impl<'a, T: AsRef<[u8]>> Unpin for StrideStream<'a, T> {}
impl<'a, T: AsRef<[u8]>> StrideStream<'a, T> {
    /// Processes all items from `buffer` into `processed`
    ///
    /// Deals handling stride and byte concat
    fn process_buffer(&mut self) {
        // Ret empty if buffer is empty or insufficient buffer size
        // or processed enough
        if self.element_processed >= self.element_count || self.buffer.len() < self.element_stride {
            return;
        }
        let iter = self.buffer.chunks_exact(self.element_stride);
        let remainder: Vec<u8> = iter.remainder().to_vec();
        for stride_chunk in iter {
            self.processed
                .extend_from_slice(&stride_chunk[0..self.element_size]);
            self.element_processed += 1;
            // stop if we processed more than expected
            if self.element_processed >= self.element_count {
                break;
            }
        }
        // Discard any remaining bytes back to be used later or discarded
        self.buffer = remainder;
    }

    /// Prepares the buffer to be yielded
    fn get_yielded_data(&mut self) -> Option<anyhow::Result<Vec<u8>>> {
        if self.processed.is_empty() {
            None
        } else {
            let end = self.frame_size.min(self.processed.len());
            Some(Ok(self.processed.drain(0..end).collect::<Vec<u8>>()))
        }
    }
}

impl<'a, T: AsRef<[u8]>> futures_core::Stream for StrideStream<'a, T> {
    type Item = anyhow::Result<Vec<u8>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        // More elements expected
        match this.data_stream.poll_next(cx) {
            Poll::Ready(data) => {
                match data {
                    None => {
                        // No more data expected from data stream
                        this.process_buffer();
                        if let Some(buf) = this.get_yielded_data() {
                            Poll::Ready(Some(buf))
                        } else if !this.processed.is_empty() {
                            // dump remaining out
                            Poll::Ready(Some(Ok(this.processed.clone())))
                        } else {
                            Poll::Ready(None)
                        }
                    }
                    Some(data) => {
                        // Data can be added
                        match data {
                            Ok(mut data) => {
                                // check offset and ret early
                                if this.bytes_recv + data.as_ref().len() < this.offset {
                                    this.bytes_recv += data.as_ref().len();
                                    cx.waker().wake_by_ref();
                                    return Poll::Pending;
                                }
                                // Add new data and process it
                                this.buffer.extend_from_slice(
                                    &data.as_ref()
                                        [this.offset.checked_sub(this.bytes_recv).unwrap_or(0)..],
                                );
                                this.bytes_recv += data.as_ref().len();
                                this.process_buffer();
                                if this.processed.len() >= this.frame_size
                                    || this.element_processed >= this.element_count
                                {
                                    Poll::Ready(this.get_yielded_data())
                                } else {
                                    cx.waker().wake_by_ref();
                                    Poll::Pending
                                }
                            }
                            Err(e) => Poll::Ready(Some(Err(e))),
                        }
                    }
                }
            }
            Poll::Pending => {
                this.process_buffer();
                if let Some(buf) = this.get_yielded_data() {
                    Poll::Ready(Some(buf))
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
        }
    }
}

unsafe impl<'a, T: AsRef<[u8]>> Send for StrideStream<'a, T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use futures::executor::LocalPool;
    use futures::stream::{self, StreamExt};
    use futures::task::LocalSpawnExt;
    use std::rc::Rc;

    #[test]
    fn test_stride_stream_basic() {
        // Parameters
        let offset = 0;
        let element_size = 2;
        let element_stride = 4;
        let element_count = 3;
        let frame_size = 4;

        // Input data: bytes from 0 to 15
        let data: Vec<u8> = (0..16).collect();

        // Create a vector of data chunks
        let data_chunks: Vec<Result<Vec<u8>>> =
            data.chunks(4).map(|chunk| Ok(chunk.to_vec())).collect();

        // Create a stream from the data chunks
        let data_stream = stream::iter(data_chunks).boxed_local();

        // Build the StrideStream
        let builder = StrideStreamBuilder {
            offset,
            element_size,
            element_stride,
            element_count,
            frame_size,
        };

        let mut stride_stream = builder.build(data_stream);

        // Collect output
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        pool.spawner()
            .spawn_local(async move {
                while let Some(Ok(chunk)) = stride_stream.next().await {
                    sender.unbounded_send(chunk).unwrap();
                }
            })
            .unwrap();

        let output_chunks: Vec<Vec<u8>> = pool.run_until(receiver.collect());

        // Expected output
        let expected_output = vec![vec![0, 1, 4, 5], vec![8, 9]];

        assert_eq!(output_chunks, expected_output);
    }

    #[test]
    fn test_stride_stream_with_offset() {
        // Parameters
        let offset = 2;
        let element_size = 2;
        let element_stride = 4;
        let element_count = 3;
        let frame_size = 4;

        // Input data
        let data: Vec<u8> = (0..16).collect();

        // Create a vector of data chunks
        let data_chunks: Vec<anyhow::Result<Vec<u8>>> =
            data.chunks(4).map(|chunk| Ok(chunk.to_vec())).collect();

        // Create a stream from the data chunks
        let data_stream = stream::iter(data_chunks).boxed_local();

        // Build the StrideStream
        let builder = StrideStreamBuilder {
            offset,
            element_size,
            element_stride,
            element_count,
            frame_size,
        };

        let mut stride_stream = builder.build(data_stream);

        // Collect output
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        pool.spawner()
            .spawn_local(async move {
                while let Some(Ok(chunk)) = stride_stream.next().await {
                    sender.unbounded_send(chunk).unwrap();
                }
            })
            .unwrap();

        let output_chunks: Vec<Vec<u8>> = pool.run_until(receiver.collect());

        // Expected output
        let expected_output = vec![vec![2, 3, 6, 7], vec![10, 11]];

        assert_eq!(output_chunks, expected_output);
    }

    #[test]
    fn test_stride_stream_element_size_eq_stride() {
        // Parameters
        let offset = 0;
        let element_size = 4;
        let element_stride = 4;
        let element_count = 3;
        let frame_size = 8;

        // Input data
        let data: Vec<u8> = (0..16).collect();

        // Create a vector of data chunks
        let data_chunks: Vec<anyhow::Result<Vec<u8>>> =
            data.chunks(4).map(|chunk| Ok(chunk.to_vec())).collect();

        // Create a stream from the data chunks
        let data_stream = stream::iter(data_chunks).boxed_local();

        // Build the StrideStream
        let builder = StrideStreamBuilder {
            offset,
            element_size,
            element_stride,
            element_count,
            frame_size,
        };

        let mut stride_stream = builder.build(data_stream);

        // Collect output
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        pool.spawner()
            .spawn_local(async move {
                while let Some(Ok(chunk)) = stride_stream.next().await {
                    sender.unbounded_send(chunk).unwrap();
                }
            })
            .unwrap();

        let output_chunks: Vec<Vec<u8>> = pool.run_until(receiver.collect());

        // Expected output
        let expected_output = vec![vec![0, 1, 2, 3, 4, 5, 6, 7], vec![8, 9, 10, 11]];

        assert_eq!(output_chunks, expected_output);
    }
}
