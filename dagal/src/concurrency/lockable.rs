use anyhow::Result;
use std::ops::{Deref, DerefMut};
/// Abstracts over multiple concurrency libraries

pub trait Lockable {
    type Lock<'a>: Deref<Target = Self::Target> + DerefMut + 'a
    where
        Self: 'a;
    type Target: ?Sized;
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum TryLockError {
    #[error("Poison error")]
    PoisonError,
    #[error("Lock is not available")]
    WouldBlock,
}

pub trait SyncLockable: Lockable {
    fn lock<'a>(&'a self) -> Result<Self::Lock<'a>>;

    /// Attempts to get a lock
    ///
    /// If enc
    fn try_lock<'a>(&'a self) -> Result<Option<Self::Lock<'a>>>;
}

pub trait AsyncLockable: Lockable {
    async fn lock<'a>(&'a self) -> Result<Self::Lock<'a>>;

    /// Attempts to get a lock
    ///
    /// If enc
    async fn try_lock<'a>(&'a self) -> Result<Option<Self::Lock<'a>>>;
}
