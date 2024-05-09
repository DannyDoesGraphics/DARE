pub enum MemoryType {
    /// Memory useful in device accessible memory
    GpuOnly,
    /// Memory useful for uploading data to the device
    CpuToGpu,
    /// Memory useful for read back of data
    GpuToCpu,
}
