/// Describes the state of a gpu resource
#[derive(Debug)]
pub enum GPUResource<T> {
    NotLoaded,
    Loading,
    Loaded(T),
    Unloading,
}