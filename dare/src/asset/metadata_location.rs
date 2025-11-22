use std::sync::Arc;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum MetaDataLocation {
    Url(String),
    FilePath(std::path::PathBuf),
    Memory(Arc<[u8]>),
}
