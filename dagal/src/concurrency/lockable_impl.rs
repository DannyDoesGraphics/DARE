/// Concerning implementations of Mute x
pub use super::lockable::*;
use crate::DagalError::PoisonError;

impl<T> Lockable for std::sync::Mutex<T> {
    type Lock<'a> = std::sync::MutexGuard<'a, T> where T: 'a;
    type Target = T;
}

impl<T> SyncLockable for std::sync::Mutex<T> {
    fn lock<'a>(&'a self) -> anyhow::Result<Self::Lock<'a>> {
        Ok(self.lock().map_err(|_| anyhow::Error::from(PoisonError))?)
    }

    fn try_lock<'a>(&'a self) -> anyhow::Result<Option<Self::Lock<'a>>> {
        use std::sync::TryLockError;
        match self.try_lock() {
            Ok(guard) => Ok(Some(guard)),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Poisoned(_)) => Err(anyhow::Error::from(PoisonError)),
        }
    }
}

#[cfg(feature = "tokio")]
impl<T> Lockable for tokio::sync::Mutex<T> {
    type Lock<'a> = tokio::sync::MutexGuard<'a, Self::Target> where T: 'a;
    type Target = T;
}
#[cfg(feature = "tokio")]
impl<T> AsyncLockable for tokio::sync::Mutex<T> {
    async fn lock<'a>(&'a self) -> anyhow::Result<Self::Lock<'a>> {
        Ok(self.lock().await)
    }

    async fn try_lock<'a>(&'a self) -> anyhow::Result<Option<Self::Lock<'a>>> {
        match self.try_lock() {
            Ok(guard) => Ok(Some(guard)),
            Err(e) => Ok(None),
        }
    }
}
