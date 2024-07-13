use std::marker::PhantomData;

use crate::render::backable::Backable;
use crate::render::growable_buffer::GrowableBuffer;

/// Represents a generic which can be into a Vec<T> which is backed by a [`GrowableBuffer`]
pub struct BackedGrowable<E, T: Backable<E>> {
    growable: GrowableBuffer,
    pub handle: T,
    _marker: PhantomData<E>,
}
