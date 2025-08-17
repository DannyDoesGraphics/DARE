use anyhow::Result;
use std::ops::{Deref, DerefMut};
/// Abstracts over multiple concurrency libraries

pub trait Lockable {
    type Lock<'a>: Deref<Target = Self::Target> + DerefMut + 'a
    where
        Self: 'a;
    type Target: ?Sized;

    fn new(t: Self::Target) -> Self;
}

#[derive(thiserror::Error, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TryLockError {
    #[error("Poison error")]
    PoisonError,
    #[error("Lock is not available")]
    WouldBlock,
}

pub trait SyncLockable: Lockable + TryLockable {
    fn lock(&self) -> Result<Self::Lock<'_>>;
}

pub trait AsyncLockable: Lockable + TryLockable {
    async fn lock<'a>(&'a self) -> Result<Self::Lock<'a>>;

    fn blocking_lock(&self) -> Result<Self::Lock<'_>>;
}

pub trait TryLockable: Lockable {
    fn try_lock(&self) -> Result<Self::Lock<'_>>;
}
