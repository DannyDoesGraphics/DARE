/// Describes the state of a gpu resource
#[derive(Debug)]
pub enum GPUResource<U, T> {
    NotLoaded(U),
    Loading,
    Loaded(T),
    Unloading,
}
