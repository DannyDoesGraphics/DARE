use super::*;
use anyhow::Result;
use futures::stream::{self, StreamExt};

#[cfg(test)]
mod tests {
    use super::*;
    use rand;
    use rand::Rng;

    // Test when element_size equals element_stride (no stride)
    #[tokio::test]
    async fn test_no_stride() {
        let element_size = 2;
        let element_stride = 2; // No stride
        let element_count = 3;
        let frame_size = 4; // Max size per frame

        let input_data: Vec<u8> = vec![1, 2, 3, 4, 5, 6];

        // Our data stream will yield the data in chunks
        let data_stream = stream::iter(vec![
            Ok(&input_data[0..2]),
            Ok(&input_data[2..4]),
            Ok(&input_data[4..6]),
        ])
        .boxed_local();

        let mut stride_stream = StrideStreamBuilder {
            offset: 0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        }
        .build(data_stream);

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result.unwrap());
        }

        // Expected data is the same as input data
        let expected_frames = vec![vec![1, 2, 3, 4], vec![5, 6]];

        assert_eq!(outputs, expected_frames);
    }

    // Test when element_size less than element_stride (with stride)
    #[tokio::test]
    async fn test_with_stride() {
        let element_size = 2;
        let element_stride = 3; // Skip 1 byte between elements
        let element_count = 3;
        let frame_size = 4; // Max size per frame

        // Input data: elements interleaved with stride bytes
        // Elements: [1,2], skip 3, [4,5], skip 6, [7,8]
        let input_data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];

        // Create data stream
        let data_stream = stream::iter(vec![
            Ok(&input_data[0..3]), // [1,2,3]
            Ok(&input_data[3..6]), // [4,5,6]
            Ok(&input_data[6..8]), // [7,8]
        ])
        .boxed_local();

        let mut stride_stream = StrideStreamBuilder {
            offset: 0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        }
        .build(data_stream);

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result.unwrap());
        }

        // Expected processed elements: [1,2], [4,5], [7,8]
        let expected_frames = vec![vec![1, 2, 4, 5], vec![7, 8]];

        assert_eq!(outputs, expected_frames);
    }

    // Test when element_size greater than element_stride (overlapping elements)
    #[tokio::test]
    async fn test_overlapping_elements() {
        let element_size = 3;
        let element_stride = 2; // Overlapping elements
        let element_count = 3;
        let frame_size = 6;

        let input_data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];

        let data_stream = stream::iter(vec![
            Ok(&input_data[0..4]), // [1,2,3,4]
            Ok(&input_data[4..8]), // [5,6,7,8]
        ])
        .boxed_local();

        let mut stride_stream = StrideStreamBuilder {
            offset: 0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        }
        .build(data_stream);

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result.unwrap());
        }

        // Processed elements would be:
        // - First element: [1,2,3]
        // - Second element starts at offset 2: [3,4,5]
        // - Third element starts at offset 4: [5,6,7]
        // Expected frames:
        let expected_frames = vec![vec![1, 2, 3, 3, 4, 5], vec![5, 6, 7]];

        assert_eq!(outputs, expected_frames);
    }

    // Test when data stream ends before expected elements are processed
    #[tokio::test]
    async fn test_insufficient_data() {
        let element_size = 2;
        let element_stride = 3;
        let element_count = 4;
        let frame_size = 4;

        let input_data: Vec<u8> = vec![1, 2, 3, 4, 5]; // Not enough data

        let data_stream =
            stream::iter(vec![Ok(&input_data[0..2]), Ok(&input_data[2..5])]).boxed_local();

        let mut stride_stream = StrideStreamBuilder {
            offset: 0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        }
        .build(data_stream);

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result.unwrap());
        }

        // Expected outputs might be incomplete
        let expected_frames = vec![vec![1, 2, 4, 5]];

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

        let data_stream = stream::iter(vec![Ok(&input_data[0..5])]).boxed_local();

        let mut stride_stream = StrideStreamBuilder {
            offset: 0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        }
        .build(data_stream);

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result.unwrap());
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
        let frame_size = 4;

        let input_data: Vec<u8> = vec![1, 2, 3, 4, 5, 6];

        let data_stream = stream::iter(vec![
            Ok(&input_data[0..2]),
            Err(anyhow::anyhow!("Test error")),
            Ok(&input_data[2..6]),
        ])
        .boxed_local();

        let mut stride_stream = StrideStreamBuilder {
            offset: 0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        }
        .build(data_stream);

        let mut outputs = Vec::new();
        while let Some(result) = stride_stream.next().await {
            outputs.push(result);
        }

        // Expected outputs:
        // First data: [1,2]
        // Then error
        // Then remaining data: [3,4,5,6]
        let expected_frames = vec![
            Ok(vec![1, 2]),
            Err(anyhow::anyhow!("Test error")),
            Ok(vec![3, 4, 5, 6]),
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
        let frame_size = 4096;

        // Calculate the total data size
        let total_data_size = element_stride * element_count;

        // Generate random data
        let mut rng = rand::thread_rng();
        let mut input_data = Vec::with_capacity(total_data_size);

        for _ in 0..element_count {
            // Generate an element of random bytes
            let element: Vec<u8> = (0..element_size).map(|_| rng.gen()).collect();
            input_data.extend_from_slice(&element);

            // Add padding/stride bytes (could be random or zeros)
            let padding: Vec<u8> = vec![0u8; element_stride - element_size];
            input_data.extend_from_slice(&padding);
        }

        // Simulate streaming the data in chunks
        let chunk_size = 8192; // Size of chunks to simulate streaming data
        let chunks: Vec<Result<&[u8]>> = input_data
            .chunks(chunk_size)
            .map(|chunk| Ok(chunk))
            .collect();

        // Create a data stream from the chunks
        let data_stream = stream::iter(chunks).boxed_local();

        // Initialize the StrideStream
        let mut stride_stream = StrideStreamBuilder {
            offset: 0,
            element_size,
            element_stride,
            element_count,
            frame_size,
        }
        .build(data_stream);

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
