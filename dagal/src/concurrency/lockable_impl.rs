/// Concerning implementations of Mute x
pub use super::lockable::*;
use crate::DagalError::PoisonError;

impl<T> Lockable for std::sync::Mutex<T> {
    type Lock<'a> = std::sync::MutexGuard<'a, T> where T: 'a;
    type Target = T;

    fn new(t: Self::Target) -> Self {
        std::sync::Mutex::new(t)
    }
}

impl<T> SyncLockable for std::sync::Mutex<T> {
    fn lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        self.lock().map_err(|_| anyhow::Error::from(PoisonError))
    }

    fn try_lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        self
            .try_lock()
            .map_err(|_| anyhow::Error::from(PoisonError))
    }
}

#[cfg(feature = "tokio")]
impl<T> Lockable for tokio::sync::Mutex<T> {
    type Lock<'a> = tokio::sync::MutexGuard<'a, Self::Target> where T: 'a;
    type Target = T;

    fn new(t: Self::Target) -> Self {
        tokio::sync::Mutex::new(t)
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

    fn try_lock(&self) -> anyhow::Result<Self::Lock<'_>> {
        Ok(self.try_lock()?)
    }
}
