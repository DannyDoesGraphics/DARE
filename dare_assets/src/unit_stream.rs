
use futures_util::Stream;
use futures_util::*;

use crate::format::format_convert;

/// Responsible for taking in a stream of unstructured bytes, and shaping them into [Format; chunk_size] chunks streamed out
#[derive(Debug, Clone, Hash)]
pub struct ByteStreamReshaper<InStream: Stream<Item = In> + Unpin, In: AsRef<[u8]>> {
    incoming_stream: InStream,
    incoming_format: crate::Format,
    max_elements: u64,
    /// If [`None`], always try to send regardless of how many elements can make up a chunk
    chunk_elements: Option<u64>,
    /// If [`None`], outgoing bytes are assumed to be the same as incoming, and any transmutation step is unnecessary
    outgoing_format: Option<crate::Format>,

    // stateful
    buffered_bytes: Vec<u8>,
    end_of_stream: bool,
}

impl<InStream: Stream<Item = In> + Unpin, In: AsRef<[u8]>> ByteStreamReshaper<InStream, In> {
    fn new(
        stream: InStream,
        incoming: crate::Format,
        max_elements: u64,
        elements_in_chunk: Option<u64>,
        outgoing: Option<crate::Format>,
    ) -> Self {
        Self {
            incoming_stream: stream,
            max_elements,
            chunk_elements: elements_in_chunk,
            incoming_format: incoming,
            outgoing_format: outgoing,

            buffered_bytes: Vec::new(),
            end_of_stream: false,
        }
    }

    fn apply_format_conversion(&self, bytes: Vec<u8>) -> Vec<u8> {
        if let Some(out_fmt) = self.outgoing_format {
            format_convert(self.incoming_format, out_fmt, bytes)
        } else {
            bytes
        }
    }

    fn take_input_elems(&mut self, elems: usize) -> Vec<u8> {
        let n = elems
            .checked_mul(self.incoming_format.size_in_bytes())
            .expect("Chunk byte overflow");
        self.buffered_bytes.drain(..n).collect()
    }

    fn decide_emit(&mut self) -> Option<Vec<u8>> {
        if self.max_elements == 0 {
            None
        } else {
            let buffered_elems: usize = self
                .buffered_bytes
                .len()
                .div_euclid(self.incoming_format.size_in_bytes());
            let to_remove = (buffered_elems as u64).min(self.max_elements);
            self.max_elements = self.max_elements.saturating_sub(to_remove);
            if to_remove == 0 {
                None
            } else if let Some(buffered) = self.chunk_elements
                && buffered_elems > buffered as usize
            {
                Some(self.take_input_elems(to_remove as usize))
            } else if self.chunk_elements.is_none() {
                Some(self.take_input_elems(to_remove as usize))
            } else {
                None
            }
        }
    }
}

impl<InStream: Stream<Item = In> + Unpin, In: AsRef<[u8]>> Stream
    for ByteStreamReshaper<InStream, In>
{
    type Item = Vec<u8>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            if self.end_of_stream
                && let Some(out) = self.decide_emit()
            {
                let converted = self.apply_format_conversion(out);
                return std::task::Poll::Ready(Some(converted));
            } else if self.end_of_stream {
                return std::task::Poll::Ready(None);
            } else {
                match self.incoming_stream.poll_next_unpin(cx) {
                    std::task::Poll::Ready(Some(piece)) => {
                        let b: &[u8] = piece.as_ref();
                        if !b.is_empty() {
                            self.buffered_bytes.extend_from_slice(b);
                        }
                        if let Some(elems) = self.decide_emit() {
                            let converted = self.apply_format_conversion(elems);
                            return std::task::Poll::Ready(Some(converted));
                        }
                    }
                    std::task::Poll::Ready(None) => {
                        // end of input stream
                        self.end_of_stream = true;
                        if self.chunk_elements.is_none() {
                            let result = self.decide_emit().map(|out| self.apply_format_conversion(out));
                            return std::task::Poll::Ready(result);
                        }
                    }
                    std::task::Poll::Pending => {
                        return std::task::Poll::Pending;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Format;
    use futures::executor::LocalPool;
    use futures::stream::{self, StreamExt};
    use futures::task::LocalSpawnExt;

    #[test]
    fn test_u8_to_u8_identity() {
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
        pool.spawner().spawn_local(async move {
            let input_data = vec![vec![1u8, 2, 3, 4, 5]];
            let stream = stream::iter(input_data);
            
            let mut reshaper = ByteStreamReshaper::new(
                stream,
                Format::U8,
                5,
                None,
                None,
            );
            
            while let Some(chunk) = reshaper.next().await {
                sender.unbounded_send(chunk).unwrap();
            }
        }).unwrap();
        
        let result: Vec<Vec<u8>> = pool.run_until(receiver.collect());
        let flattened: Vec<u8> = result.into_iter().flatten().collect();
        
        assert_eq!(flattened, vec![1u8, 2, 3, 4, 5]);
    }

    #[test]
    fn test_u8_to_u16() {
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
        pool.spawner().spawn_local(async move {
            let input_data = vec![vec![1u8, 2, 255]];
            let stream = stream::iter(input_data);
            
            let mut reshaper = ByteStreamReshaper::new(
                stream,
                Format::U8,
                3,
                None,
                Some(Format::U16),
            );
            
            while let Some(chunk) = reshaper.next().await {
                sender.unbounded_send(chunk).unwrap();
            }
        }).unwrap();
        
        let result: Vec<Vec<u8>> = pool.run_until(receiver.collect());
        let flattened: Vec<u8> = result.into_iter().flatten().collect();
        
        let expected_u16: Vec<u16> = vec![1, 2, 255];
        let mut expected_bytes = Vec::new();
        for val in expected_u16 {
            expected_bytes.extend_from_slice(&val.to_ne_bytes());
        }
        
        assert_eq!(flattened, expected_bytes);
    }

    #[test]
    fn test_u8_to_f32() {
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
        pool.spawner().spawn_local(async move {
            let input_data = vec![vec![0u8, 128, 255]];
            let stream = stream::iter(input_data);
            
            let mut reshaper = ByteStreamReshaper::new(
                stream,
                Format::U8,
                3,
                None,
                Some(Format::F32),
            );
            
            while let Some(chunk) = reshaper.next().await {
                sender.unbounded_send(chunk).unwrap();
            }
        }).unwrap();
        
        let result: Vec<Vec<u8>> = pool.run_until(receiver.collect());
        let flattened: Vec<u8> = result.into_iter().flatten().collect();
        
        let expected_f32: Vec<f32> = vec![0.0, 128.0, 255.0];
        let mut expected_bytes = Vec::new();
        for val in expected_f32 {
            expected_bytes.extend_from_slice(&val.to_ne_bytes());
        }
        
        assert_eq!(flattened, expected_bytes);
    }

    #[test]
    fn test_f32_to_u32() {
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
        pool.spawner().spawn_local(async move {
            let input_f32: Vec<f32> = vec![0.0, 42.5, 1000.7];
            let mut input_bytes = Vec::new();
            for val in input_f32 {
                input_bytes.extend_from_slice(&val.to_ne_bytes());
            }
            
            let stream = stream::iter(vec![input_bytes]);
            
            let mut reshaper = ByteStreamReshaper::new(
                stream,
                Format::F32,
                3,
                None,
                Some(Format::U32),
            );
            
            while let Some(chunk) = reshaper.next().await {
                sender.unbounded_send(chunk).unwrap();
            }
        }).unwrap();
        
        let result: Vec<Vec<u8>> = pool.run_until(receiver.collect());
        let flattened: Vec<u8> = result.into_iter().flatten().collect();
        
        let expected_u32: Vec<u32> = vec![0, 43, 1001];
        let mut expected_bytes = Vec::new();
        for val in expected_u32 {
            expected_bytes.extend_from_slice(&val.to_ne_bytes());
        }
        
        assert_eq!(flattened, expected_bytes);
    }

    #[test]
    fn test_f32_to_u8() {
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
        pool.spawner().spawn_local(async move {
            let input_f32: Vec<f32> = vec![0.0, 128.3, 300.5, 255.0];
            let mut input_bytes = Vec::new();
            for val in input_f32 {
                input_bytes.extend_from_slice(&val.to_ne_bytes());
            }
            
            let stream = stream::iter(vec![input_bytes]);
            
            let mut reshaper = ByteStreamReshaper::new(
                stream,
                Format::F32,
                4,
                None,
                Some(Format::U8),
            );
            
            while let Some(chunk) = reshaper.next().await {
                sender.unbounded_send(chunk).unwrap();
            }
        }).unwrap();
        
        let result: Vec<Vec<u8>> = pool.run_until(receiver.collect());
        let flattened: Vec<u8> = result.into_iter().flatten().collect();
        
        assert_eq!(flattened, vec![0u8, 128, 255, 255]);
    }

    #[test]
    fn test_f32_to_u8_max_elements_limit() {
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
        pool.spawner().spawn_local(async move {
            let input_f32: Vec<f32> = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0];
            let mut input_bytes = Vec::new();
            for val in input_f32 {
                input_bytes.extend_from_slice(&val.to_ne_bytes());
            }
            
            let stream = stream::iter(vec![input_bytes]);
            
            let mut reshaper = ByteStreamReshaper::new(
                stream,
                Format::F32,
                3,
                None,
                Some(Format::U8),
            );
            
            while let Some(chunk) = reshaper.next().await {
                sender.unbounded_send(chunk).unwrap();
            }
        }).unwrap();
        
        let result: Vec<Vec<u8>> = pool.run_until(receiver.collect());
        let flattened: Vec<u8> = result.into_iter().flatten().collect();
        
        assert_eq!(flattened, vec![10u8, 20, 30]);
        assert_eq!(flattened.len(), 3);
    }

    #[test]
    fn test_u8x4_to_f32x3() {
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
        pool.spawner().spawn_local(async move {
            let input_data = vec![
                10u8, 20, 30, 40,
                50, 60, 70, 80,
                100, 110, 120, 130,
            ];
            
            let stream = stream::iter(vec![input_data]);
            
            let mut reshaper = ByteStreamReshaper::new(
                stream,
                Format::U8x4,
                3,
                None,
                Some(Format::F32x3),
            );
            
            while let Some(chunk) = reshaper.next().await {
                sender.unbounded_send(chunk).unwrap();
            }
        }).unwrap();
        
        let result: Vec<Vec<u8>> = pool.run_until(receiver.collect());
        let flattened: Vec<u8> = result.into_iter().flatten().collect();
        
        let expected: Vec<f32> = vec![
            10.0, 20.0, 30.0,
            50.0, 60.0, 70.0,
            100.0, 110.0, 120.0,
        ];
        let mut expected_bytes = Vec::new();
        for val in expected {
            expected_bytes.extend_from_slice(&val.to_ne_bytes());
        }
        
        assert_eq!(flattened, expected_bytes);
    }

    #[test]
    fn test_f32x4_to_u8() {
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
        pool.spawner().spawn_local(async move {
            let input_data: Vec<f32> = vec![
                127.7, 20.0, 30.0, 40.0,
                300.5, 110.0, 120.0, 130.0,
                200.4, 210.0, 220.0, 230.0,
            ];
            let mut input_bytes = Vec::new();
            for val in input_data {
                input_bytes.extend_from_slice(&val.to_ne_bytes());
            }
            
            let stream = stream::iter(vec![input_bytes]);
            
            let mut reshaper = ByteStreamReshaper::new(
                stream,
                Format::F32x4,
                3,
                None,
                Some(Format::U8),
            );
            
            while let Some(chunk) = reshaper.next().await {
                sender.unbounded_send(chunk).unwrap();
            }
        }).unwrap();
        
        let result: Vec<Vec<u8>> = pool.run_until(receiver.collect());
        let flattened: Vec<u8> = result.into_iter().flatten().collect();
        
        assert_eq!(flattened, vec![128u8, 255, 200]);
    }

    #[test]
    fn test_f32x4_to_u8x3() {
        let mut pool = LocalPool::new();
        let (sender, receiver) = futures::channel::mpsc::unbounded();
        
        pool.spawner().spawn_local(async move {
            let input_data: Vec<f32> = vec![
                127.3, 255.9, 30.5, 40.0,
                100.7, 300.0, 120.1, 130.0,
                200.6, 0.4, 220.0, 230.0,
            ];
            let mut input_bytes = Vec::new();
            for val in input_data {
                input_bytes.extend_from_slice(&val.to_ne_bytes());
            }
            
            let stream = stream::iter(vec![input_bytes]);
            
            let mut reshaper = ByteStreamReshaper::new(
                stream,
                Format::F32x4,
                3,
                None,
                Some(Format::U8x3),
            );
            
            while let Some(chunk) = reshaper.next().await {
                sender.unbounded_send(chunk).unwrap();
            }
        }).unwrap();
        
        let result: Vec<Vec<u8>> = pool.run_until(receiver.collect());
        let flattened: Vec<u8> = result.into_iter().flatten().collect();
        
        assert_eq!(flattened, vec![
            127u8, 255, 31,
            101, 255, 120,
            201, 0, 220,
        ]);
    }
}
