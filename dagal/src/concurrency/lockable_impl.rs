/// Concerning components of Mute x
pub use super::lockable::*;
use crate::DagalError::PoisonError;
use std::sync::Mutex;

impl<T> Lockable for std::sync::Mutex<T> {
    type Lock<'a> = std::sync::MutexGuard<'a, T>
    where
        T: 'a;
    type Target = T;

    fn new(t: Self::Target) -> Self {
        std::sync::Mutex::new(t)
    }
}

impl<T> TryLockable for Mutex<T> {
    fn try_lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        self.try_lock()
            .map_err(|_| anyhow::Error::from(PoisonError))
    }
}

impl<T> SyncLockable for std::sync::Mutex<T> {
    fn lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        self.lock().map_err(|_| anyhow::Error::from(PoisonError))
    }
}

#[cfg(feature = "tokio")]
impl<T> Lockable for tokio::sync::Mutex<T> {
    type Lock<'a> = tokio::sync::MutexGuard<'a, Self::Target>
    where
        T: 'a;
    type Target = T;

    fn new(t: Self::Target) -> Self {
        tokio::sync::Mutex::new(t)
    }
}

#[cfg(feature = "tokio")]
impl<T> TryLockable for tokio::sync::Mutex<T> {
    fn try_lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        self.try_lock()
            .map_err(|_| anyhow::Error::from(PoisonError))
    }
}

#[cfg(feature = "tokio")]
impl<T> AsyncLockable for tokio::sync::Mutex<T> {
    async fn lock<'a>(&'a self) -> anyhow::Result<Self::Lock<'a>> {
        Ok(self.lock().await)
    }

    fn blocking_lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        Ok(self.blocking_lock())
    }
}

#[cfg(feature = "futures")]
impl<T> Lockable for futures::lock::Mutex<T> {
    type Lock<'a>
    where
        Self: 'a,
    = futures::lock::MutexGuard<'a, T>;
    type Target = T;

    fn new(t: Self::Target) -> Self {
        futures::lock::Mutex::new(t)
    }
}

#[cfg(feature = "futures")]
impl<T> TryLockable for futures::lock::Mutex<T> {
    fn try_lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        self.try_lock()
            .map_or(Err(anyhow::anyhow!("Unable to acquire lock")), Ok)
    }
}

#[cfg(feature = "futures")]
impl<T> AsyncLockable for futures::lock::Mutex<T> {
    async fn lock<'a>(&'a self) -> anyhow::Result<Self::Lock<'a>> {
        Ok(self.lock().await)
    }

    fn blocking_lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        Ok(self.blocking_lock()?)
    }
}

#[cfg(feature = "async-std")]
impl<T> Lockable for async_std::sync::Mutex<T> {
    type Lock<'a>
    where
        Self: 'a,
    = async_std::sync::MutexGuard<'a, T>;
    type Target = T;

    fn new(t: Self::Target) -> Self {
        async_std::sync::Mutex::new(t)
    }
}

#[cfg(feature = "async-std")]
impl<T> TryLockable for async_std::sync::Mutex<T> {
    fn try_lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        self.try_lock()
            .map_or(Err(anyhow::anyhow!("Unable to acquire lock")), Ok)
    }
}

#[cfg(feature = "async-std")]
impl<T> AsyncLockable for async_std::sync::Mutex<T> {
    async fn lock<'a>(&'a self) -> anyhow::Result<Self::Lock<'a>> {
        Ok(self.lock().await)
    }

    fn blocking_lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        Ok(self.blocking_lock()?)
    }
}
