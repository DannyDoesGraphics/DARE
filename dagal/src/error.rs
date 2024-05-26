use std::sync::PoisonError;
/// Possible errors
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DagalError {
    #[error("No window was provided")]
    NoWindow,

    #[error("It is impossible to create requested queue")]
    ImpossibleQueue,

    #[error("No suitable physical device has been found")]
    NoPhysicalDevice,

    #[error("Poisoned mutex")]
    PoisonError,

    #[error("Did not query struct ahead of time")]
    NoQuery,

    #[error("No capabilities were provided")]
    NoCapabilities,

    #[error("shaderc encountered an error")]
    ShadercError,

    #[error("Expected buffer to have vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS")]
    NoShaderDeviceAddress,

    #[error("Vulkan resource does not have a mapped pointer. You're most likely using GPU only")]
    NoMappedPointer,

    #[error("Insufficient space to upload the data")]
    InsufficientSpace,
}

impl<T> From<PoisonError<T>> for DagalError {
    fn from(_: PoisonError<T>) -> Self {
        DagalError::PoisonError
    }
}