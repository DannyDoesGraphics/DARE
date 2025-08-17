use bytes::Bytes;

pub mod buffer;
pub mod info;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ByteChunk {
    pub data: Bytes,
    pub offset: usize,
}
