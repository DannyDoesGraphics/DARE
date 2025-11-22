#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum RenderAssetState {
    /// Indicates the asset is loaded
    Unloaded,
    /// Indicates the asset is initialized on the GPU, but not loaded in
    Initialized,
    /// Indicates the asset is completely loaded
    Loaded,
}
