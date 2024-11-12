use bytemuck::{NoUninit, Pod};
use futures::stream::StreamExt;
use futures_core::stream::BoxStream;
use num_traits::{Bounded, FromPrimitive, ToPrimitive};
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct CastStream<
    'a,
    T: Into<Vec<u8>>,
    Source: ToPrimitive + Default + Unpin + NoUninit + Pod,
    Destination: ToPrimitive + Default + Unpin + NoUninit + Pod + FromPrimitive,
> {
    stream: super::framer::Framer<'a, T>,
    source_dimension: usize,
    dest_dimension: usize,
    buffer: Vec<u8>,
    _phantom: PhantomData<(Source, Destination)>,
}

impl<
        'a,
        T: Into<Vec<u8>>,
        Source: ToPrimitive + Default + Unpin + NoUninit + Pod,
        Destination: ToPrimitive + Default + Unpin + NoUninit + Pod + FromPrimitive,
    > Unpin for CastStream<'a, T, Source, Destination>
{
}

impl<
        'a,
        T: Into<Vec<u8>>,
        Source: ToPrimitive + Default + Unpin + NoUninit + Pod,
        Destination: ToPrimitive + Default + Unpin + NoUninit + Pod + FromPrimitive,
    > CastStream<'a, T, Source, Destination>
{
    pub fn new(
        stream: BoxStream<'a, T>,
        in_buffer_size: usize,
        source_dim: usize,
        dest_dim: usize,
    ) -> Self {
        let min_amount = (in_buffer_size / (size_of::<Source>() * source_dim)).max(1);
        let frame_size = min_amount * size_of::<Source>() * source_dim;
        Self {
            stream: super::framer::Framer::new(stream, frame_size),
            source_dimension: source_dim,
            dest_dimension: dest_dim,
            buffer: Vec::with_capacity(frame_size),
            _phantom: PhantomData,
        }
    }
}

impl<
        'a,
        T: Into<Vec<u8>>,
        Source: ToPrimitive + Default + Unpin + NoUninit + Bounded + Pod,
        Destination: ToPrimitive + FromPrimitive + Default + Unpin + NoUninit + Bounded + Pod,
    > futures_core::Stream for CastStream<'a, T, Source, Destination>
{
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        match this.stream.poll_next_unpin(cx) {
            Poll::Ready(data) => match data {
                None => Poll::Ready(None),
                Some(data) => {
                    let source_data: &[Source] = bytemuck::cast_slice(&data);
                    let mut data: Vec<u8> = Vec::with_capacity(this.dest_dimension);
                    for sources in source_data.chunks(this.source_dimension).into_iter() {
                        let sources: Vec<Source> = {
                            let mut sources: Vec<Source> = sources.to_vec();
                            sources.resize(this.dest_dimension, Source::zeroed());
                            sources
                        };
                        let destinations: Vec<Destination> = sources
                            .iter()
                            .map(|source| {
                                match source.to_f64() {
                                    None => {}
                                    Some(prim) => match Destination::from_f64(prim) {
                                        None => {}
                                        Some(prim) => return prim,
                                    },
                                }
                                match source.to_i128() {
                                    None => {}
                                    Some(prim) => match Destination::from_i128(prim) {
                                        None => {}
                                        Some(prim) => return prim,
                                    },
                                }
                                match source.to_u128() {
                                    None => {}
                                    Some(prim) => match Destination::from_u128(prim) {
                                        None => {}
                                        Some(prim) => return prim,
                                    },
                                }
                                panic!("Unable to convert");
                            })
                            .collect::<Vec<Destination>>();

                        let mut destinations_bytes: Vec<u8> =
                            bytemuck::cast_slice(&destinations).to_vec();
                        data.append(&mut destinations_bytes);
                    }
                    Poll::Ready(Some(data))
                }
            },
            Poll::Pending => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::{cast_slice, Pod};
    use futures::stream;
    use futures::StreamExt;
    use num_traits::{Bounded, FromPrimitive, ToPrimitive};

    fn setup_stream<T: Pod>(data: Vec<T>) -> BoxStream<'static, Vec<u8>> {
        let byte_data: Vec<u8> = data
            .into_iter()
            .flat_map(|val| cast_slice(&[val]).to_vec())
            .collect();
        stream::iter(vec![byte_data]).boxed()
    }

    // Helper function to test conversions from a source type to a destination type with dimensional differences
    async fn test_cast_stream<Source, Destination>(
        source_data: Vec<Source>,
        source_dimension: usize,
        dest_dimension: usize,
    ) where
        Source: ToPrimitive + Default + Unpin + NoUninit + Pod + Bounded,
        Destination: ToPrimitive
            + Default
            + Unpin
            + NoUninit
            + Pod
            + FromPrimitive
            + Bounded
            + std::cmp::PartialEq
            + std::fmt::Debug,
    {
        // Set up the stream with casted byte data
        let stream = setup_stream(source_data.clone());

        let mut cast_stream: CastStream<_, Source, Destination> =
            CastStream::new(stream, 64, source_dimension, dest_dimension);

        // Collect results from CastStream
        let result = cast_stream.collect::<Vec<Vec<u8>>>().await;

        // Ensure we received data and validate the conversion
        assert!(!result.is_empty());

        // Process each result and validate the casting logic with dimensions in mind
        for chunk in result {
            // Interpret the chunk as a slice of Destination items
            let dest_data: &[Destination] = cast_slice(&chunk);

            for (i, dest_val) in dest_data.iter().enumerate() {
                // Determine the chunk in source_data we are processing based on dimensions
                let source_chunk_start = (i / dest_dimension) * source_dimension;
                let source_chunk_end = source_chunk_start + source_dimension;

                let source_values =
                    &source_data[source_chunk_start..source_chunk_end.min(source_data.len())];

                // For each element in source_values, convert to Destination and compare
                let mut expected_values = Vec::with_capacity(dest_dimension);

                for &source_val in source_values {
                    // Convert each Source to the Destination type, handling floating-point or integer transformations
                    let expected_val = Destination::from_f64(source_val.to_f64().unwrap()).unwrap();
                    expected_values.push(expected_val);
                }

                // Pad with zeroed Destination values if source chunk is smaller than destination dimension
                while expected_values.len() < dest_dimension {
                    expected_values.push(Destination::zeroed());
                }

                // Check each element in dest_data against the expected values
                assert_eq!(
                    *dest_val,
                    expected_values[i % dest_dimension],
                    "Mismatch in converted value at index {}",
                    i
                );
            }
        }
    }

    #[tokio::test]
    async fn test_u16_to_u64() {
        let source_data: Vec<u16> = vec![0, 1, 2, 3, 65535];
        test_cast_stream::<u16, u64>(source_data, 1, 1).await;
    }

    #[tokio::test]
    async fn test_f32_to_f64() {
        let source_data: Vec<f32> = vec![0.0, 1.0, 2.5, -3.4, f32::MAX, f32::MIN];
        test_cast_stream::<f32, f64>(source_data, 1, 1).await;
    }

    #[tokio::test]
    async fn test_i16_to_i32() {
        let source_data: Vec<i16> = vec![0, 1, -2, 32767, -32768];
        test_cast_stream::<i16, i32>(source_data, 1, 1).await;
    }

    #[tokio::test]
    async fn test_u32_to_f32() {
        let source_data: Vec<u32> = vec![0, 1, 10, 4294967295];
        test_cast_stream::<u32, f32>(source_data, 1, 1).await;
    }

    #[tokio::test]
    async fn test_u8_to_u16_with_multiple_dimensions() {
        let source_data: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8];
        test_cast_stream::<u8, u16>(source_data, 2, 1).await;
    }

    #[tokio::test]
    async fn test_f32_to_f64_with_higher_dimensions() {
        let source_data: Vec<f32> = vec![1.0, 2.0, 3.5, 4.6, -5.7];
        test_cast_stream::<f32, f64>(source_data, 2, 2).await;
    }

    #[tokio::test]
    async fn test_i32_to_f64_with_truncation() {
        let source_data: Vec<i32> = vec![-10, 20, -30, 40];
        test_cast_stream::<i32, f64>(source_data, 1, 2).await;
    }
}
