use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncSeekExt};
use bytes::*;

/// A file stream that reads data from a file 
/// 
/// Primarily provides a stream that skips n bytes and reads until m bytes have been read
#[derive(Debug)]
pub struct FileStream<'a> {
    file: tokio::fs::File,
    frame_size: usize,
    length: usize,
    bytes_read: usize,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> FileStream<'a> {
    /// Create a new file stream object from a file
    pub async fn new(
        mut file: tokio::fs::File,
        offset: usize,
        frame_size: usize,
        length: usize,
    ) -> Result<Self, std::io::Error> {
        file.seek(std::io::SeekFrom::Start(offset as u64)).await?;
        Ok(Self {
            file,
            frame_size,
            length,
            bytes_read: 0,
            _phantom: Default::default(),
        })
    }

    /// Create a new file stream object from a path
    pub async fn from_path(
        path: &std::path::Path,
        offset: usize,
        frame_size: usize,
        length: usize,
    ) -> Result<Self, std::io::Error> {
        let mut file = tokio::fs::File::open(path).await?;
        file.seek(std::io::SeekFrom::Start(offset as u64)).await?;
        Ok(Self {
            file,
            frame_size,
            length,
            bytes_read: 0,
            _phantom: Default::default(),
        })
    }
}

impl<'a> Deref for FileStream<'a> {
    type Target = tokio::fs::File;

    fn deref(&self) -> &Self::Target {
        &self.file
    }
}

impl<'a> DerefMut for FileStream<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.file
    }
}

impl<'a> futures::Stream for FileStream<'a> {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        let remaining = this.length.saturating_sub(this.bytes_read);
        let mut buffer = vec![0; this.frame_size];
        let mut read_buf = tokio::io::ReadBuf::new(&mut buffer);
        match Pin::new(&mut this.file).poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(())) => {
                let filled_data = read_buf.filled();
                if filled_data.is_empty() {
                    Poll::Ready(None)
                } else {
                    let vec = filled_data[0..remaining.min(filled_data.len())].to_vec();
                    this.bytes_read += vec.len();
                    //println!("loading file: {:?}", filled_data);
                    Poll::Ready(Some(Ok(Bytes::from(vec))))
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}
