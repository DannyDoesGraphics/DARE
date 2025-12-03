/// A structure representing geometric data in the asset system.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Geometry {
    pub location: super::DataLocation,
    pub format: super::Format,
    pub offset: u64,
    /// If None, data is tightly packed
    pub stride: Option<u64>,
    pub max_size: u64,
    pub count: u64,
}

impl Geometry {
    /// Ensure that count * stride does not exceed max_size AND (element size * count) < max_size
    pub fn validate(&self) -> bool {
        (self.count * self.stride.unwrap_or(1)) <= self.max_size
            && (self.format.size_in_bytes() as u64 * self.count) <= self.max_size
    }
}
