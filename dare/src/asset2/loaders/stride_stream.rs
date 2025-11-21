use bevy_tasks::futures_lite::StreamExt;
use bytes::{Bytes, BytesMut};
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
    pub fn build<'a, T: Into<Bytes>>(
        self,
        stream: futures::stream::LocalBoxStream<'a, anyhow::Result<T>>,
    ) -> StrideStream<'a, T> {
        StrideStream {
            offset: self.offset,
            element_size: self.element_size,
            element_stride: self.element_stride,
            element_count: self.element_count,
            frame_size: self.frame_size,
            data_stream: stream,
            element_processed: 0,
            bytes_recv: 0,
            processed: BytesMut::with_capacity(self.frame_size),
            buffer: BytesMut::new(),
        }
    }
}

/// StrideStream is a post process stream that collects all data from an incoming [`futures::stream::LocalBoxStream`]
///
/// # Cancellation safety
/// As long as StrideStream is kept alive outside the cancellation scope,
/// it is cancellation safe as it keeps state
#[derive(Derivative)]
#[derivative(Debug)]
pub struct StrideStream<'a, T: Into<Bytes>> {
    offset: usize,
    element_size: usize,
    element_stride: usize,
    element_count: usize,
    frame_size: usize,

    #[derivative(Debug = "ignore")]
    /// We do not expect data_stream to be processed on another thread
    data_stream: futures::stream::LocalBoxStream<'a, anyhow::Result<T>>,

    /// \# of elements processed
    element_processed: usize,
    /// \# of bytes received from the stream (for offset)
    bytes_recv: usize,
    /// Bytes of the processed elements
    /// waiting to the yielded out
    processed: BytesMut,
    /// Bytes of elements awaiting processing
    buffer: BytesMut,
}
impl<'a, T: Into<Bytes>> Unpin for StrideStream<'a, T> {}
impl<'a, T: Into<Bytes>> StrideStream<'a, T> {
    /// Processes all items from `buffer` into `processed`
    ///
    /// Deals handling stride and byte concat
    fn process_buffer(&mut self) {
        // Ret empty if buffer is empty or insufficient buffer size
        // or processed enough
        while self.element_processed < self.element_count
            && self.buffer.len() >= self.element_stride
        {
            let chunk = self.buffer.split_to(self.element_stride);
            self.processed
                .extend_from_slice(&chunk[..self.element_size]);
            self.element_processed += 1;
        }
    }

    /// Prepares the buffer to be yielded
    fn get_yielded_data(&mut self) -> Option<anyhow::Result<Bytes>> {
        if self.processed.is_empty() {
            None
        } else {
            let end = self.frame_size.min(self.processed.len());
            Some(Ok(self.processed.split_to(end).freeze()))
        }
    }
}

impl<'a, T: Into<Bytes>> futures::Stream for StrideStream<'a, T> {
    type Item = anyhow::Result<Bytes>;

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
                            let remaining =
                                this.processed.split_to(this.processed.len()).freeze();
                            Poll::Ready(Some(Ok(remaining)))
                        } else {
                            Poll::Ready(None)
                        }
                    }
                    Some(data) => {
                        // Data can be added
                        match data {
                            Ok(data) => {
                                let data: Bytes = data.into();
                                let data_len = data.len();
                                // check offset and ret early
                                if this.bytes_recv + data_len < this.offset {
                                    this.bytes_recv += data_len;
                                    cx.waker().wake_by_ref();
                                    return Poll::Pending;
                                }
                                // Add new data and process it
                                let start = this
                                    .offset
                                    .saturating_sub(this.bytes_recv)
                                    .min(data_len);
                                if start < data_len {
                                    this.buffer
                                        .extend_from_slice(&data.as_ref()[start..]);
                                }
                                this.bytes_recv += data_len;
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

unsafe impl<'a, T: Into<Bytes>> Send for StrideStream<'a, T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use futures::executor::LocalPool;
    use futures::stream::{self, StreamExt};
    use futures::task::LocalSpawnExt;

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
        let data_chunks: Vec<Result<Bytes>> = data
            .chunks(4)
            .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
            .collect();

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

        let output_chunks: Vec<Bytes> = pool.run_until(receiver.collect());
        let output_chunks: Vec<Vec<u8>> =
            output_chunks.into_iter().map(|bytes| bytes.to_vec()).collect();

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
        let data_chunks: Vec<anyhow::Result<Bytes>> = data
            .chunks(4)
            .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
            .collect();

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

        let output_chunks: Vec<Bytes> = pool.run_until(receiver.collect());
        let output_chunks: Vec<Vec<u8>> =
            output_chunks.into_iter().map(|bytes| bytes.to_vec()).collect();

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
        let data_chunks: Vec<anyhow::Result<Bytes>> = data
            .chunks(4)
            .map(|chunk| Ok(Bytes::copy_from_slice(chunk)))
            .collect();

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

        let output_chunks: Vec<Bytes> = pool.run_until(receiver.collect());
        let output_chunks: Vec<Vec<u8>> =
            output_chunks.into_iter().map(|bytes| bytes.to_vec()).collect();

        // Expected output
        let expected_output = vec![vec![0, 1, 2, 3, 4, 5, 6, 7], vec![8, 9, 10, 11]];

        assert_eq!(output_chunks, expected_output);
    }
}
