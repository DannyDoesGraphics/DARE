use futures::stream::StreamExt;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A framer's entire job is to guarantee stream yields are within [`Self::frame_size`] or less
///
/// This is cancellation safe so long as the input stream is as well
pub struct Framer<'a, T: Into<Vec<u8>>> {
    stream: futures_core::stream::BoxStream<'a, T>,
    frame_size: usize,
    buffer: Vec<u8>,
}
impl<'a, T: Into<Vec<u8>>> Unpin for Framer<'a, T> {}
impl<'a, T: Into<Vec<u8>>> Framer<'a, T> {
    pub fn new(stream: futures_core::stream::BoxStream<'a, T>, frame_size: usize) -> Self {
        Self {
            stream,
            frame_size,
            buffer: Vec::with_capacity(frame_size),
        }
    }

    fn get_next_frame(self: &mut Self) -> Option<Vec<u8>> {
        if self.buffer.len() >= self.frame_size {
            Some(self.buffer.drain(0..self.frame_size).collect())
        } else {
            None
        }
    }
}

impl<'a, T: Into<Vec<u8>>> futures_core::stream::Stream for Framer<'a, T> {
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.frame_size == 0 {
            // no frames should be made for size zero
            return Poll::Ready(None);
        }

        match this.stream.poll_next_unpin(cx) {
            Poll::Pending => match this.get_next_frame() {
                Some(frame) => Poll::Ready(Some(frame.to_vec())),
                None => match this.get_next_frame() {
                    Some(frame) => Poll::Ready(Some(frame)),
                    None => {
                        // keep polling for more
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                },
            },
            Poll::Ready(data) => match data {
                Some(data) => {
                    let mut data: Vec<u8> = data.into();
                    this.buffer.append(&mut data);
                    match this.get_next_frame() {
                        None => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                        Some(frame) => Poll::Ready(Some(frame)),
                    }
                }
                None => match this.get_next_frame() {
                    Some(frame) => Poll::Ready(Some(frame)),
                    None => {
                        if !this.buffer.is_empty() {
                            Poll::Ready(Some(this.buffer.drain(..).collect()))
                        } else {
                            Poll::Ready(None)
                        }
                    }
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use futures::{stream, StreamExt};
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    #[test]
    fn test_single_large_chunk() {
        let data = vec![1u8; 100];
        let stream = stream::iter(vec![data.clone()]);
        let boxed_stream = stream.boxed();
        let frame_size = 10;
        let mut framer = Framer::new(boxed_stream, frame_size);

        let mut frames = Vec::new();
        block_on(async {
            while let Some(frame) = framer.next().await {
                frames.push(frame);
            }
        });

        // We should have 10 frames of size 10
        assert_eq!(frames.len(), 10);
        for frame in frames {
            assert_eq!(frame.len(), 10);
        }
    }

    #[test]
    fn test_multiple_small_chunks() {
        let data = vec![vec![1u8; 3], vec![2u8; 4], vec![3u8; 5]];
        let stream = stream::iter(data);
        let boxed_stream = stream.boxed();
        let frame_size = 5;
        let mut framer = Framer::new(boxed_stream, frame_size);

        let mut frames = Vec::new();
        block_on(async {
            while let Some(frame) = framer.next().await {
                frames.push(frame);
            }
        });

        // Expected frames:
        // First frame: 3 bytes of 1s + 2 bytes of 2s
        // Second frame: 2 bytes of 2s + 3 bytes of 3s
        // Third frame: 2 bytes of 3s

        assert_eq!(frames.len(), 3);
        assert_eq!(
            frames[0],
            vec![1u8; 3]
                .into_iter()
                .chain(vec![2u8; 2])
                .collect::<Vec<u8>>()
        );
        assert_eq!(
            frames[1],
            vec![2u8; 2]
                .into_iter()
                .chain(vec![3u8; 3])
                .collect::<Vec<u8>>()
        );
        assert_eq!(frames[2], vec![3u8; 2]);
    }

    #[test]
    fn test_exact_frame_sizes() {
        let data = vec![vec![1u8; 5], vec![2u8; 5], vec![3u8; 5]];
        let stream = stream::iter(data);
        let boxed_stream = stream.boxed();
        let frame_size = 5;
        let mut framer = Framer::new(boxed_stream, frame_size);

        let mut frames = Vec::new();
        block_on(async {
            while let Some(frame) = framer.next().await {
                frames.push(frame);
            }
        });

        // Should have frames of exact size
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], vec![1u8; 5]);
        assert_eq!(frames[1], vec![2u8; 5]);
        assert_eq!(frames[2], vec![3u8; 5]);
    }

    #[test]
    fn test_empty_stream() {
        let stream = stream::iter(Vec::<Vec<u8>>::new());
        let boxed_stream = stream.boxed();
        let frame_size = 5;
        let mut framer = Framer::new(boxed_stream, frame_size);

        let mut frames = Vec::new();
        block_on(async {
            while let Some(frame) = framer.next().await {
                frames.push(frame);
            }
        });

        // Should have no frames
        assert_eq!(frames.len(), 0);
    }

    #[test]
    fn test_remaining_data() {
        let data = vec![vec![1u8; 2], vec![2u8; 2], vec![3u8; 2]];
        let stream = stream::iter(data);
        let boxed_stream = stream.boxed();
        let frame_size = 5;
        let mut framer = Framer::new(boxed_stream, frame_size);

        let mut frames = Vec::new();
        block_on(async {
            while let Some(frame) = framer.next().await {
                frames.push(frame);
            }
        });

        // Should have one frame of size 5 and one frame of size 1
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].len(), 5);
        assert_eq!(frames[1].len(), 1);
    }

    #[test]
    fn test_stream_with_pending() {
        struct PendingOnceStream<T> {
            data: Option<T>,
            pending_once: bool,
        }

        impl<T> PendingOnceStream<T> {
            fn new(data: T) -> Self {
                Self {
                    data: Some(data),
                    pending_once: true,
                }
            }
        }

        impl<T: std::marker::Unpin> stream::Stream for PendingOnceStream<T> {
            type Item = T;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let this = self.get_mut();
                if this.pending_once {
                    this.pending_once = false;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                } else {
                    Poll::Ready(this.data.take())
                }
            }
        }

        let data = vec![1u8; 10];
        let stream = PendingOnceStream::new(data.clone());
        let boxed_stream = stream.boxed();
        let frame_size = 5;
        let mut framer = Framer::new(boxed_stream, frame_size);

        let mut frames = Vec::new();
        block_on(async {
            while let Some(frame) = framer.next().await {
                frames.push(frame);
            }
        });

        // Should have two frames of size 5
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], vec![1u8; 5]);
        assert_eq!(frames[1], vec![1u8; 5]);
    }

    #[test]
    fn test_large_frame_size() {
        let data = vec![vec![1u8; 3], vec![2u8; 4], vec![3u8; 5]];
        let stream = stream::iter(data);
        let boxed_stream = stream.boxed();
        let frame_size = 20;
        let mut framer = Framer::new(boxed_stream, frame_size);

        let mut frames = Vec::new();
        block_on(async {
            while let Some(frame) = framer.next().await {
                frames.push(frame);
            }
        });

        // All data should be in a single frame
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].len(), 12);
    }

    #[test]
    fn test_zero_frame_size() {
        let data = vec![vec![1u8; 5], vec![2u8; 5]];
        let stream = stream::iter(data);
        let boxed_stream = stream.boxed();

        // Frame size zero should be handled gracefully
        let frame_size = 0;
        let mut framer = Framer::new(boxed_stream, frame_size);

        let mut frames: Vec<u8> = Vec::new();
        block_on(async {
            if let Some(_frame) = framer.next().await {
                // Should not reach here
                assert!(false, "Frame size zero should not produce frames");
            }
        });

        // Should have no frames
        assert_eq!(frames.len(), 0);
    }
}
