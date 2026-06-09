use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;

/// Reads a file in fixed-size chunks for [`dare_assets::ByteStreamReshaper`].
#[derive(Debug)]
pub struct FileByteStream<R: Read + Unpin, const N: usize> {
    reader: R,
    scratch: [u8; N],
}

impl<const N: usize> FileByteStream<File, N> {
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Ok(Self {
            reader: File::open(path)?,
            scratch: [0; N],
        })
    }
}

impl<R: Read + Unpin, const N: usize> FileByteStream<R, N> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            scratch: [0; N],
        }
    }
}

impl<R: Read + Unpin, const N: usize> Stream for FileByteStream<R, N> {
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match this.reader.read(&mut this.scratch) {
            Ok(0) => Poll::Ready(None),
            Ok(n) => Poll::Ready(Some(this.scratch[..n].to_vec())),
            Err(err) => {
                tracing::error!(?err, "file read failed");
                Poll::Ready(None)
            }
        }
    }
}
