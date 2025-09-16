use dagal::allocators::Allocator;


/// A standard ring buffer allocator using a single host and device visible buffer
#[derive(Debug)]
pub struct RingBuffer<A: Allocator, T: Sized> {
    buffer: dagal::resource::Buffer<A>,
    read: u64,
    write: u64,
    _marker: std::marker::PhantomData<T>,
}

impl<A: Allocator, T> RingBuffer<A, T> {
    pub fn new(buffer: dagal::resource::Buffer<A>) -> Self {
        Self {
            buffer,
            read: 0,
            write: 0,
            _marker: std::marker::PhantomData,
        }
    }

    /// Write to ring buffer and return the position of the new write head
    pub fn write(&mut self, data: &[T]) -> Result<u64, dagal::DagalError> {
        let size = std::mem::size_of_val(data) as u64;
        let buffer_size = self.buffer.get_size();
        
        // Check if there's enough space (simplified check)
        if size > buffer_size {
            return Err(dagal::DagalError::InsufficientSpace);
        }
        
        let end_space = buffer_size - self.write;
        if size <= end_space {
            // Can write contiguously
            self.buffer.write(self.write, data)?;
        } else {
            // Need to wrap around
            let first_part_len = (end_space / std::mem::size_of::<T>() as u64) as usize;
            self.buffer.write(self.write, &data[0..first_part_len])?;
            self.buffer.write(0, &data[first_part_len..])?;
        }
        
        self.write = (self.write + size) % buffer_size;
        Ok(self.write)
    }

    /// Reads `amount` of `T` from the buffer at the current read head, and returns a pointer to the read data in
    /// the buffer.
    /// 
    /// # Invariance
    /// Does not support reading across the ring buffer wrap-around as it would require an expensive clone operation.
    pub fn read(&mut self, amount: u64) -> Result<&[T], dagal::DagalError> {
        // wrap the read around if necessary
        let size = std::mem::size_of::<T>() as u64 * amount;
        if self.read + size > self.buffer.get_size() {
            return Err(dagal::DagalError::InsufficientSpace);
        }
        let out = self.buffer.read(self.read, amount)?;
        self.read = (self.read + size) % self.buffer.get_size();
        Ok(out)
    }

    pub fn write_head(&self) -> u64 {
        self.write
    }

    pub fn read_head(&self) -> u64 {
        self.read
    }
}