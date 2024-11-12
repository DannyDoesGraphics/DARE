use futures::stream::{BoxStream, Stream, StreamExt};
use num_traits::{Bounded, NumCast};
use std::marker::Unpin;
use std::pin::Pin;
use std::task::{Context, Poll};

trait FromBytes: Sized {
    fn from_bytes(bytes: &[u8]) -> Self;
}

macro_rules! impl_from_bytes {
    ($($t:ty),*) => {
        $(
            impl FromBytes for $t {
                fn from_bytes(bytes: &[u8]) -> Self {
                    let mut array = [0u8; std::mem::size_of::<Self>()];
                    array.copy_from_slice(bytes);
                    Self::from_le_bytes(array)
                }
            }
        )*
    };
}

impl_from_bytes!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

pub struct ByteStreamCaster<S, Source, Target>
where
    S: Stream + Unpin,
    S::Item: AsRef<[u8]>,
    Source: FromBytes + Copy + PartialOrd + NumCast + Bounded,
    Target: Copy + PartialOrd + NumCast + Bounded,
{
    stream: BoxStream<'static, Vec<u8>>,
    buffer: Vec<u8>,
    _marker: std::marker::PhantomData<(S, Source, Target)>,
}

impl<S, Source, Target> ByteStreamCaster<S, Source, Target>
where
    S: Stream + Unpin + Send + 'static,
    S::Item: AsRef<[u8]> + Send + 'static,
    Source: FromBytes + Copy + PartialOrd + NumCast + Bounded + Send + 'static,
    Target: Copy + PartialOrd + NumCast + Bounded + Send + 'static,
{
    pub fn new(stream: S) -> Self {
        Self {
            stream: stream.map(|item| item.as_ref().to_vec()).boxed(),
            buffer: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }

    fn poll_read_exact(
        &mut self,
        cx: &mut Context<'_>,
        n: usize,
    ) -> Poll<Result<(), std::io::Error>> {
        while self.buffer.len() < n {
            match self.stream.poll_next_unpin(cx) {
                Poll::Ready(Some(chunk)) => {
                    self.buffer.extend_from_slice(&chunk);
                }
                Poll::Ready(None) => {
                    if self.buffer.is_empty() {
                        // End of stream and buffer is empty
                        return Poll::Ready(Ok(()));
                    } else {
                        // End of stream but buffer has insufficient data
                        return Poll::Ready(Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "End of stream",
                        )));
                    }
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
        Poll::Ready(Ok(()))
    }

    fn read_value(&mut self) -> Result<Source, std::io::Error> {
        let n = std::mem::size_of::<Source>();
        let bytes = self.buffer.drain(..n).collect::<Vec<u8>>();
        let value = Source::from_bytes(&bytes);
        Ok(value)
    }

    fn cast_value(&self, source_value: Source) -> Result<Target, std::io::Error> {
        // Convert Target's min and max values to Source type for comparison
        let target_min: Source = NumCast::from(Target::min_value()).unwrap_or(Source::min_value());
        let target_max: Source = NumCast::from(Target::max_value()).unwrap_or(Source::max_value());

        // Clamp the source value if necessary
        let clamped_source = if source_value < target_min {
            target_min
        } else if source_value > target_max {
            target_max
        } else {
            source_value
        };

        // Convert the clamped source value to the target type
        let target_value: Target = NumCast::from(clamped_source).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Failed to cast clamped source value to target type",
            )
        })?;

        Ok(target_value)
    }
}

impl<S, Source, Target> Stream for ByteStreamCaster<S, Source, Target>
where
    S: Stream + Unpin + Send + 'static,
    S::Item: AsRef<[u8]> + Send + 'static,
    Source: FromBytes + Copy + PartialOrd + NumCast + Bounded + Send + 'static + Unpin,
    Target: Copy + PartialOrd + NumCast + Bounded + Send + 'static + Unpin,
{
    type Item = Result<Target, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let n = size_of::<Source>();

        match this.poll_read_exact(cx, n) {
            Poll::Ready(Ok(())) => {
                if this.buffer.len() < n {
                    // End of stream and no more data
                    return Poll::Ready(None);
                }
                match this.read_value() {
                    Ok(source_value) => match this.cast_value(source_value) {
                        Ok(target_value) => Poll::Ready(Some(Ok(target_value))),
                        Err(e) => Poll::Ready(Some(Err(e))),
                    },
                    Err(e) => Poll::Ready(Some(Err(e))),
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}
