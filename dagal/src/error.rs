use std::sync::PoisonError;

/// Possible errors
use thiserror::Error;

#[derive(Debug, Error, Copy, Clone, PartialEq, Eq, Hash)]
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

    #[error("Invalid slot map slot used")]
    InvalidSlotMapSlot,

    #[error("Current memory allocation is empty/freed")]
    EmptyMemoryAllocation,

    #[error("No backing buffer found")]
    NoSuperBuffer,

    #[error("Extension is not supported or enabled")]
    NoExtensionSupported,

    #[error("GPU Resource Table does has no strong references to the slot")]
    NoStrongReferences,
}

impl<T> From<PoisonError<T>> for DagalError {
    fn from(_: PoisonError<T>) -> Self {
        DagalError::PoisonError
    }
}
