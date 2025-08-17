use anyhow::Result;
use std::ops::{Deref, DerefMut};
/// Abstracts over multiple concurrency libraries

/// Trait representing a guard that provides read-only access to protected data
pub trait Guard<T: ?Sized>: Deref<Target = T> {}

/// Trait representing a guard that provides mutable access to protected data
pub trait MutableGuard<T: ?Sized>: Guard<T> + DerefMut<Target = T> {}

impl<G, T: ?Sized> Guard<T> for G where G: Deref<Target = T> {}
impl<G, T: ?Sized> MutableGuard<T> for G where G: Deref<Target = T> + DerefMut<Target = T> {}

pub trait Lockable {
    type Lock<'a>: MutableGuard<Self::Target> + 'a
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
