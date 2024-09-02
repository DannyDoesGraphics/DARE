/// Describes a basic data structure where data are pooled into chunks which are then streamed

/// Representation of a chunk of data
struct DataChunk<T> {
    data: Vec<T>,
}

impl<T> DataChunk<T> {
    fn new(data: Vec<T>) -> Self {
        DataChunk {
            data,
        }
    }

    /// Number of elements in the data chunk
    fn elements(&self) -> usize {
        self.data.len()
    }
}

pub struct Pool<T> {}