
/// A "frame" is simply a snapshot of the current world and provides a list of candidate meshes to render
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    // Pairs of [`super::AssetManager`] and their respective [`super::MeshHandle`] in the manager for book keeping
    pub meshes: Vec<()>,
}