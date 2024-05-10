pub trait Semaphore<'a> {
    type Handle: Copy + Clone;
    /// Get a reference to the underlying [`VkSemaphore`](vk::Semaphore)
    fn get_handle(&self) -> Self::Handle;
    /// Get a copy to the underlying [`VkSemaphore`](vk::Semaphore)
    fn handle(&self) -> Self::Handle;
    /// Get a reference of the device used by the semaphore
    fn get_device(&self) -> &'a crate::device::LogicalDevice;
}
