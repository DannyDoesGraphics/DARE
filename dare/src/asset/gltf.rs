use anyhow::Result;

/// Handles gltf loading
pub struct GLTFLoader {}

impl GLTFLoader {
    pub fn new() -> Self {
        Self {}
    }

    pub fn load(path: std::path::PathBuf) -> Result<()> {
        Ok(())
    }
}