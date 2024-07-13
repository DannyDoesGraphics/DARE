/// Indicates a reference back with an offset into a buffer when copying
pub struct BackedPtrLocations<'a, T> {
    pub handle: &'a T,
    pub offset: usize,
}

/// Struct can be backed by a buffer
pub trait Backable<T> {
    fn get_backed_elements(&self) -> &[BackedPtrLocations<T>];
}