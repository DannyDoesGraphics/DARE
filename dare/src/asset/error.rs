use thiserror::Error;

#[derive(Debug, Error, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AssetErrors {
    #[error("Expected a loaded asset, got unloaded asset")]
    AssetNotLoaded,
    #[error("Expected metadata, got None")]
    AssetMetadataNone,
}