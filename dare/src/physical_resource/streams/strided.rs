use std::{pin::Pin, task::Poll};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::{StreamExt, TryStreamExt, stream::LocalBoxStream};

/// Builds a stride stream
///
/// A strided stream will produce a frame of n elements specified. It will buffer up to n * elem_stride bytes
/// then attempt to process the buffered data accounting for the element stride and size.
///
///
/// It will then output a frame of the processed bytes of n * elem_size.
///
/// # Offset
/// Offsets are simply calculated by skipping over n seen bytes where n is offset.
pub struct StridedStream<'a> {
    pub stream: LocalBoxStream<'a, anyhow::Result<Bytes>>,
    /// Offset to drop bytes from
    pub offset: usize,
    /// Size of each element
    pub elem_size: usize,
    /// Stride of each element
    pub elem_stride: usize,
    /// The max # of elements to read
    pub max_elem_count: usize,
    /// Counts the max # of elements in an output frame
    pub max_elements_in_output: usize,
    /// \# of seen bytes from input stream (for offset skipping)
    seen: usize,
    /// \# of elements fully copied so far (across all frames)
    processed_elems: usize,
    /// \# of elements sent out
    out_elems: usize,
    /// Accumulator for al processed elements
    out: BytesMut,
}

impl<'a> StridedStream<'a> {
    pub fn new(
        stream: LocalBoxStream<'a, anyhow::Result<Bytes>>,
        offset: usize,
        elem_size: usize,
        elem_stride: usize,
        max_elem_count: usize,
        out_frame_count: usize,
    ) -> Self {
        debug_assert!(
            elem_size <= elem_stride,
            "Element size must be less than or equal to stride"
        );
        debug_assert!(elem_size > 0, "Element size must be greater than zero");
        debug_assert!(
            max_elem_count > 0,
            "Max element count must be greater than zero"
        );
        debug_assert!(
            out_frame_count > 0,
            "Max elements in output frame must be greater than zero"
        );
        Self {
            stream,
            offset,
            elem_size,
            elem_stride,
            max_elem_count,
            max_elements_in_output: out_frame_count,
            seen: 0,
            out_elems: 0,
            processed_elems: 0,
            out: BytesMut::with_capacity(out_frame_count.saturating_mul(elem_size)),
        }
    }

    #[inline]
    fn emit_ready_frame(&mut self, flush_remaining: bool) -> Option<Bytes> {
        let available: usize = self.processed_elems.saturating_sub(self.out_elems);
        if available == 0 {
            return None;
        }

        // mid-stream: only emit when enough data for a full frame
        // eos: emit any remaining valid full stride data
        let need: usize = if flush_remaining {
            1
        } else {
            self.max_elements_in_output
        };
        if available < need {
            return None;
        }

        let emit_elems = available
            .min(self.max_elements_in_output)
            .min(self.max_elem_count.saturating_sub(self.out_elems));
        if emit_elems == 0 {
            return None;
        }
        let emit_len = emit_elems.saturating_mul(self.elem_size);
        debug_assert!(self.out.len() >= available * self.elem_size);
        debug_assert!(self.out.len() - available * self.elem_size < self.elem_size);

        self.out_elems += emit_elems;
        Some(self.out.split_to(emit_len).freeze())
    }

    #[inline]
    fn process_incoming_buffer(&mut self, chunk: Bytes) {
        let mut i: usize = 0;
        if self.seen < self.offset {
            let need: usize = (self.offset - self.seen).min(chunk.len());
            self.seen += need;
            i += need;
        }
        // Processed all
        if i >= chunk.len() || self.processed_elems >= self.max_elem_count {
            return;
        }

        while i < chunk.len() && self.processed_elems < self.max_elem_count {
            let without_offset: usize = self.seen.saturating_sub(self.offset);
            let pos_in_stride: usize = without_offset % self.elem_stride;

            // inside element portion of stride
            if pos_in_stride < self.elem_size {
                let need_data: usize = self.elem_size - pos_in_stride;
                let take: usize = need_data.min(chunk.len() - i);
                self.out.extend_from_slice(&chunk[i..i + take]);
                self.seen += take;
                i += take;

                // If there's no padding, finishing data completes the stride
                if take == need_data {
                    self.processed_elems += 1;
                }
                if i >= chunk.len() {
                    break;
                }
            }

            // skip anything remaining in the stride
            let without_offset: usize = self.seen.saturating_sub(self.offset);
            let pos_in_stride: usize = without_offset % self.elem_stride;
            if pos_in_stride >= self.elem_size {
                let pad_rem = self.elem_stride - pos_in_stride;
                let take = pad_rem.min(chunk.len() - i);
                self.seen += take;
                i += take;
            }
            // if we hit max elements in the middle of a chunk, we can skip the rest
            if self.processed_elems >= self.max_elem_count && i < chunk.len() {
                self.seen += chunk.len() - i;
                break;
            }
        }
    }
}

impl<'a> futures::stream::Stream for StridedStream<'a> {
    type Item = anyhow::Result<Bytes>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // check if we can emit something
        if let Some(frame) = this.emit_ready_frame(false) {
            return Poll::Ready(Some(Ok(frame)));
        }
        // prevent beyond max elements
        if this.out_elems >= this.max_elem_count {
            this.out.clear();
            return Poll::Ready(None);
        }

        match this.stream.poll_next_unpin(cx) {
            Poll::Ready(buf) => match buf {
                Some(Ok(bytes)) => {
                    if !bytes.is_empty() {
                        this.process_incoming_buffer(bytes);
                    }
                    cx.waker().wake_by_ref();
                    if let Some(frame) = this.emit_ready_frame(false) {
                        Poll::Ready(Some(Ok(frame)))
                    } else {
                        Poll::Pending
                    }
                }
                Some(Err(e)) => Poll::Ready(Some(Err(e))),
                None => {
                    if let Some(frame) = this.emit_ready_frame(true) {
                        Poll::Ready(Some(Ok(frame)))
                    } else {
                        Poll::Ready(None)
                    }
                }
            },
            Poll::Pending => {
                if let Some(frame) = this.emit_ready_frame(false) {
                    Poll::Ready(Some(Ok(frame)))
                } else {
                    Poll::Pending
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use rand::RngCore;

    // Test when element_size equals element_stride (no stride)
    #[tokio::test]
    async fn test_no_stride() {
        let element_size = 2;
        let element_stride = 2; // No stride
        let element_count = 3;
        let max_elements_out = 3; // Max elements per frame

        let input_data: Vec<u8> = vec![1, 2, 3, 4, 5, 6];

        // Our data stream will yield the data in chunks
        let data_stream = futures::stream::iter(vec![
            Ok::<_, anyhow::Error>(bytes::Bytes::copy_from_slice(&input_data[0..2])),
            Ok::<_, anyhow::Error>(bytes::Bytes::copy_from_slice(&input_data[2..4])),
            Ok::<_, anyhow::Error>(bytes::Bytes::copy_from_slice(&input_data[4..6])),
        ])
        .boxed_local();

        let mut stride_stream = StridedStream::new(
            data_stream,
            0,
            element_size,
            element_stride,
            element_count,
            max_elements_out,
        );

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result.unwrap().to_vec());
        }

        // Expected data is the same as input data
        let expected_frames = vec![vec![1, 2, 3, 4, 5, 6]];

        assert_eq!(outputs, expected_frames);
    }

    // Test when element_size less than element_stride (with stride)
    #[tokio::test]
    async fn test_with_stride() {
        let element_size = 2;
        let element_stride = 3; // Skip 1 byte between elements
        let element_count = 3;
        let frame_size = 3; // Max size per frame

        // Input data: elements interleaved with stride bytes
        // Elements: [1,2], skip 3, [4,5], skip 6, [7,8]
        let input_data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];

        // Create data stream
        let data_stream = futures::stream::iter(vec![
            Ok::<_, anyhow::Error>(bytes::Bytes::copy_from_slice(&input_data[0..3])), // [1,2,3]
            Ok::<_, anyhow::Error>(bytes::Bytes::copy_from_slice(&input_data[3..6])), // [4,5,6]
            Ok::<_, anyhow::Error>(bytes::Bytes::copy_from_slice(&input_data[6..8])), // [7,8]
        ])
        .boxed_local();

        let mut stride_stream = StridedStream::new(
            data_stream,
            0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        );

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result.unwrap().to_vec());
        }

        assert_eq!(outputs, vec![vec![1, 2, 4, 5, 7, 8]]);
    }

    // Test when data stream ends before expected elements are processed
    #[tokio::test]
    async fn test_insufficient_data() {
        let element_size = 2;
        let element_stride = 3;
        let element_count = 4;
        let frame_size = 1;

        let input_data: Vec<u8> = vec![1, 2, 3, 4, 5]; // Not enough data

        let data_stream = futures::stream::iter(vec![
            Ok::<_, anyhow::Error>(bytes::Bytes::copy_from_slice(&input_data[0..2])),
            Ok::<_, anyhow::Error>(bytes::Bytes::copy_from_slice(&input_data[2..5])),
        ])
        .boxed_local();

        let mut stride_stream = StridedStream::new(
            data_stream,
            0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        );

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result.unwrap().to_vec());
        }

        // Expected outputs might be incomplete
        let expected_frames = vec![vec![1, 2], vec![4, 5]];

        assert_eq!(outputs, expected_frames);
    }

    // Test when frame size limits output
    #[tokio::test]
    async fn test_frame_size_limit() {
        let element_size = 1;
        let element_stride = 1;
        let element_count = 5;
        let frame_size = 2;

        let input_data: Vec<u8> = vec![10, 20, 30, 40, 50];

        let data_stream = futures::stream::iter(vec![Ok::<_, anyhow::Error>(
            bytes::Bytes::copy_from_slice(&input_data[0..5]),
        )])
        .boxed_local();

        let mut stride_stream = StridedStream::new(
            data_stream,
            0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        );

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result.unwrap().to_vec());
        }

        // Expected outputs are chunks of frame_size
        let expected_frames = vec![vec![10, 20], vec![30, 40], vec![50]];

        assert_eq!(outputs, expected_frames);
    }

    // Test when data stream returns errors
    #[tokio::test]
    async fn test_data_stream_errors() {
        let element_size = 2;
        let element_stride = 2;
        let element_count = 3;
        let frame_size = 3;

        let input_data: Vec<u8> = vec![1, 2, 3, 4, 5, 6];

        // [[1,2], [err], [3,4,5,6]]
        let data_stream = futures::stream::iter(vec![
            Ok::<_, anyhow::Error>(bytes::Bytes::copy_from_slice(&input_data[0..2])),
            Err(anyhow::anyhow!("Test error")),
            Ok::<_, anyhow::Error>(bytes::Bytes::copy_from_slice(&input_data[2..6])),
        ])
        .boxed_local();

        let mut stride_stream = StridedStream::new(
            data_stream,
            0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        );

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result.map(|b| b.to_vec()));
        }

        // Expected outputs:
        // Then error
        // Then remaining data: [1,2,3,4]
        let expected_frames = vec![
            Err(anyhow::anyhow!("Test error")),
            Ok(vec![1, 2, 3, 4, 5, 6]),
        ];

        // Compare outputs with expected frames
        assert_eq!(outputs.len(), expected_frames.len());
        for (output, expected) in outputs.iter().zip(expected_frames.iter()) {
            match (output, expected) {
                (Ok(o_data), Ok(e_data)) => assert_eq!(o_data, e_data),
                (Err(o_err), Err(e_err)) => assert_eq!(o_err.to_string(), e_err.to_string()),
                _ => panic!("Mismatch between output and expected"),
            }
        }
    }

    #[tokio::test]
    async fn test_large_random_data() {
        // Parameters for the test
        let element_size = 4;
        let element_stride = 6;
        let element_count = 100_000;
        let frame_size = 128;

        // Calculate the total data size
        let total_data_size = element_stride * element_count;

        // Generate random data
        let mut rng = rand::rng();
        let mut input_data = Vec::with_capacity(total_data_size);

        for _ in 0..element_count {
            // Generate an element of random bytes
            let element: Vec<u8> = (0..element_size).map(|_| rng.next_u32() as u8).collect();
            input_data.extend_from_slice(&element);

            // Add padding/stride bytes (could be random or zeros)
            let padding: Vec<u8> = vec![0u8; element_stride - element_size];
            input_data.extend_from_slice(&padding);
        }

        // Simulate streaming the data in chunks
        let chunk_size = 8192; // Size of chunks to simulate streaming data
        let chunks: Vec<anyhow::Result<bytes::Bytes>> = input_data
            .chunks(chunk_size)
            .map(|chunk| Ok(bytes::Bytes::copy_from_slice(chunk)))
            .collect();

        // Create a data stream from the chunks
        let data_stream = futures::stream::iter(chunks).boxed_local();

        // Initialize the StrideStream
        let mut stride_stream = StridedStream::new(
            data_stream,
            0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        );

        // Collect outputs from the StrideStream
        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            match result {
                Ok(data) => outputs.extend_from_slice(&data),
                Err(e) => panic!("Error occurred during streaming: {}", e),
            }
        }

        // Extract expected elements from input_data for verification
        let mut expected_data = Vec::with_capacity(element_size * element_count);
        for i in 0..element_count {
            let start = i * element_stride;
            let end = start + element_size;
            expected_data.extend_from_slice(&input_data[start..end]);
        }

        // Verify that the outputs match the expected data
        assert_eq!(outputs.len(), expected_data.len());
        assert_eq!(outputs, expected_data);
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;
    use futures::{stream, Stream, StreamExt};
    use std::io;
    use tokio_util::{codec::FramedRead, io::StreamReader};

    // Helper to wire a chunk stream -> StreamReader -> FramedRead
    fn framed_from_chunks<S>(
        chunks: S,
        codec: StridedCodec,
    ) -> FramedRead<StreamReader<S, Bytes>, StridedCodec>
    where
        S: Stream<Item = Result<Bytes, io::Error>>,
    {
        let reader = StreamReader::new(chunks);
        FramedRead::new(reader, codec)
    }

    // element_size == element_stride (no stride)
    #[tokio::test]
    async fn test_no_stride() {
        let element_size = 2;
        let element_stride = 2;
        let element_count = 3;
        let frame_size = 4;

        let input: Vec<u8> = vec![1, 2, 3, 4, 5, 6];
        let chunks = vec![
            Ok(Bytes::copy_from_slice(&input[0..2])),
            Ok(Bytes::copy_from_slice(&input[2..4])),
            Ok(Bytes::copy_from_slice(&input[4..6])),
        ];

        let codec = StridedCodec {
            offset: 0,
            elem_size: element_size,
            elem_stride: element_stride,
            elem_count: element_count,
            frame_size,
            seen: 0,
            taken: 0,
            out: BytesMut::with_capacity(frame_size),
        };

        let mut frames = framed_from_chunks(stream::iter(chunks), codec);

        let mut outputs: Vec<Vec<u8>> = Vec::new();
        while let Some(item) = frames.next().await {
            outputs.push(item.unwrap().to_vec());
        }

        let expected = vec![vec![1, 2, 3, 4], vec![5, 6]];
        assert_eq!(outputs, expected);
    }

    // element_size < element_stride (skip bytes between elements)
    #[tokio::test]
    async fn test_with_stride() {
        let element_size = 2;
        let element_stride = 3;
        let element_count = 3;
        let frame_size = 4;

        // bytes: [1,2,3, 4,5,6, 7,8]
        let input: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let chunks = vec![
            Ok(Bytes::copy_from_slice(&input[0..3])),
            Ok(Bytes::copy_from_slice(&input[3..6])),
            Ok(Bytes::copy_from_slice(&input[6..8])),
        ];

        let codec = StridedCodec {
            offset: 0,
            elem_size: element_size,
            elem_stride: element_stride,
            elem_count: element_count,
            frame_size,
            seen: 0,
            taken: 0,
            out: BytesMut::with_capacity(frame_size),
        };

        let mut frames = framed_from_chunks(stream::iter(chunks), codec);
        let mut outputs = Vec::new();
        while let Some(item) = frames.next().await {
            outputs.push(item.unwrap().to_vec());
        }

        // Last stride is incomplete at EOF, so only first two elements are emitted.
        let expected = vec![vec![1, 2, 4, 5]];
        assert_eq!(outputs, expected);
    }

    // Stream ends before all expected elements processed
    #[tokio::test]
    async fn test_insufficient_data() {
        let element_size = 2;
        let element_stride = 3;
        let element_count = 4;
        let frame_size = 4;

        let input: Vec<u8> = vec![1, 2, 3, 4, 5]; // not enough for 4 elements
        let chunks = vec![
            Ok(Bytes::copy_from_slice(&input[0..2])),
            Ok(Bytes::copy_from_slice(&input[2..5])),
        ];

        let codec = StridedCodec {
            offset: 0,
            elem_size: element_size,
            elem_stride: element_stride,
            elem_count: element_count,
            frame_size,
            seen: 0,
            taken: 0,
            out: BytesMut::with_capacity(frame_size),
        };

        let mut frames = framed_from_chunks(stream::iter(chunks), codec);
        let mut outputs = Vec::new();
        while let Some(item) = frames.next().await {
            outputs.push(item.unwrap().to_vec());
        }

        // Only the first element [1,2] is complete.
        let expected = vec![vec![1, 2]];
        assert_eq!(outputs, expected);
    }

    // frame_size limits each yield
    #[tokio::test]
    async fn test_frame_size_limit() {
        let element_size = 1;
        let element_stride = 1;
        let element_count = 5;
        let frame_size = 2;

        let input: Vec<u8> = vec![10, 20, 30, 40, 50];
        let chunks = vec![Ok(Bytes::copy_from_slice(&input[..]))];

        let codec = StridedCodec {
            offset: 0,
            elem_size: element_size,
            elem_stride: element_stride,
            elem_count: element_count,
            frame_size,
            seen: 0,
            taken: 0,
            out: BytesMut::with_capacity(frame_size),
        };

        let mut frames = framed_from_chunks(stream::iter(chunks), codec);
        let mut outputs = Vec::new();
        while let Some(item) = frames.next().await {
            outputs.push(item.unwrap().to_vec());
        }

        let expected = vec![vec![10, 20], vec![30, 40], vec![50]];
        assert_eq!(outputs, expected);
    }

    // Underlying I/O error terminates the framed stream.
    #[tokio::test]
    async fn test_data_stream_errors() {
        let element_size = 2;
        let element_stride = 2;
        let element_count = 3;
        let frame_size = 4;

        let input: Vec<u8> = vec![1, 2, 3, 4, 5, 6];
        let chunks = vec![
            Ok(Bytes::copy_from_slice(&input[0..2])),
            Err(io::Error::new(io::ErrorKind::Other, "Test error")),
            Ok(Bytes::copy_from_slice(&input[2..6])),
        ];

        let codec = StridedCodec {
            offset: 0,
            elem_size: element_size,
            elem_stride: element_stride,
            elem_count: element_count,
            frame_size,
            seen: 0,
            taken: 0,
            out: BytesMut::with_capacity(frame_size),
        };

        let mut frames = framed_from_chunks(stream::iter(chunks), codec);

        // First item is the propagated I/O error; stream terminates after.
        let first = frames.next().await.expect("one item");
        assert!(first.err().unwrap().to_string().contains("Test error"));

        // No further frames after an error.
        assert!(frames.next().await.is_none());
    }

    #[tokio::test]
    async fn test_large_random_data() {
    use rand::RngCore;

        let element_size = 4;
        let element_stride = 6;
    let element_count = 2_000_000;
        let frame_size = 4096;

        // Build interleaved (elem + padding) buffer
    let mut rng = rand::rng();
        let total = element_stride * element_count;
        let mut input = Vec::with_capacity(total);
        for _ in 0..element_count {
            // 4 data bytes
            for _ in 0..element_size {
                input.push(rng.next_u32() as u8);
            }
            // stride - element_size padding
            input.extend(std::iter::repeat(0u8).take(element_stride - element_size));
        }

        // Simulate streaming in chunks
        let chunk_size = 8192;
        let chunks = input
            .chunks(chunk_size)
            .map(|c| Ok::<_, io::Error>(Bytes::copy_from_slice(c)))
            .collect::<Vec<_>>();

        let codec = StridedCodec {
            offset: 0,
            elem_size: element_size,
            elem_stride: element_stride,
            elem_count: element_count,
            frame_size,
            seen: 0,
            taken: 0,
            out: BytesMut::with_capacity(frame_size),
        };

        let mut frames = framed_from_chunks(stream::iter(chunks), codec);

        // Concatenate all frames
        let mut out = Vec::with_capacity(element_size * element_count);
        while let Some(item) = frames.next().await {
            out.extend_from_slice(&item.unwrap());
        }

        // Expected: take the first element_size from each stride
        let mut expected = Vec::with_capacity(element_size * element_count);
        for i in 0..element_count {
            let start = i * element_stride;
            expected.extend_from_slice(&input[start..start + element_size]);
        }

        assert_eq!(out.len(), expected.len());
        assert_eq!(out, expected);
    }
}
*/
