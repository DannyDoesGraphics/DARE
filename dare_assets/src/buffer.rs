use crate::{DataLocation, Format};

#[derive(Debug, Clone)]
pub struct Buffer {
    pub location: DataLocation,
    pub format: Format,
    pub offset: u64,
    pub stride: Option<u64>,
    pub count: u64,
}

impl crate::Asset for Buffer {}
