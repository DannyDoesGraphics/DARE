/// Describes all formats supported by geometry assets
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Format {
    U16,
    U32,
    U64,
    F32,
    F64,
    F32x2,
    F32x3,
    F32x4,
    F64x2,
    F64x3,
    F64x4,
    UNKNOWN,
}

impl Format {
    pub fn size_in_bytes(&self) -> usize {
        match self {
            Format::U16 => 2,
            Format::U32 => 4,
            Format::U64 => 8,
            Format::F32 => 4,
            Format::F64 => 8,
            Format::F32x2 => 8,
            Format::F32x3 => 12,
            Format::F32x4 => 16,
            Format::F64x2 => 16,
            Format::F64x3 => 24,
            Format::F64x4 => 32,
            Format::UNKNOWN => 0,
        }
    }

    /// Size of a component in bytes
    pub fn component_size(&self) -> usize {
        match self {
            Format::U16 => 2,
            Format::U32 => 4,
            Format::U64 => 8,
            Format::F32 => 4,
            Format::F64 => 8,
            Format::F32x2 => 4,
            Format::F32x3 => 4,
            Format::F32x4 => 4,
            Format::F64x2 => 8,
            Format::F64x3 => 8,
            Format::F64x4 => 8,
            Format::UNKNOWN => 0,
        }
    }

    pub fn component_count(&self) -> usize {
        match self {
            Format::U16 => 1,
            Format::U32 => 1,
            Format::U64 => 1,
            Format::F32 => 1,
            Format::F64 => 1,
            Format::F32x2 => 2,
            Format::F32x3 => 3,
            Format::F32x4 => 4,
            Format::F64x2 => 2,
            Format::F64x3 => 3,
            Format::F64x4 => 4,
            Format::UNKNOWN => 0,
        }
    }
}
