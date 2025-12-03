pub mod assets;
mod format;
mod location;
mod manager;
mod streams;

pub use format::*;
pub use location::*;
pub use manager::*;

/// Engine-facing asset manager
#[derive(Debug)]
pub struct AssetManager {}
