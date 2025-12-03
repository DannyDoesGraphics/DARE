/// Asset handle representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetId<T> {
    id: u64,
    generation: u64,
    _marker: std::marker::PhantomData<T>,
}
