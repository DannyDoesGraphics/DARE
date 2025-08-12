#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MemoryLocation {
    /// Memory useful in device accessible memory
    GpuOnly,
    /// Memory useful for uploading data to the device
    CpuToGpu,
    /// Memory useful for read back of data
    GpuToCpu,
    /// Memory that is restricted to the host
    CpuOnly,
}
