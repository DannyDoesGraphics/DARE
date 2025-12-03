#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetLocation {
    FilePath(std::path::PathBuf),
    Url(String),
}
