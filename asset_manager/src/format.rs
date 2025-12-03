use std::str::Bytes;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Format {
    F32x4,
    F32x3,
    F32x2,
    F32,
    F64x4,
    F64x3,
    F64x2,
    F64,
    U64,
    U32,
    U8x3,
    U8,
}

impl Format {
    /// Size in bytes
    pub fn size_in_bytes(&self) -> usize {
        match self {
            Format::F32x4 => 16,
            Format::F32x3 => 12,
            Format::F32x2 => 8,
            Format::F64x4 => 32,
            Format::F64x3 => 24,
            Format::F64x2 => 16,
            Format::F32 => 4,
            Format::U64 | Format::F64 => 8,
            Format::U32 => 4,
            Format::U8x3 => 3,
            Format::U8 => 1,
        }
    }

    /// # of elements
    pub fn element_count(&self) -> usize {
        match self {
            Format::F32x4 | Format::F64x4 => 4,
            Format::F64x3 | Format::F32x3 | Format::U8x3 => 3,
            Format::F32x2 | Format::F64x2 => 2,
            Format::F64 | Format::F32 | Format::U32 | Format::U64 | Format::U8 => 1,
        }
    }

    /// Size of an element
    pub fn element_size(&self) -> usize {
        match self {
            Format::F32x4 | Format::F32x3 | Format::F32x2 | Format::F32 | Format::U32 => 4,
            Format::F64x4 | Format::F64x3 | Format::F64x2 | Format::F64 | Format::U64 => 8,
            Format::U8x3 | Format::U8 => 1,
        }
    }

    /// Get base scalar type
    pub fn scalar_type(&self) -> Format {
        match self {
            Format::F32x4 | Format::F32x3 | Format::F32x2 | Format::F32 => Format::F32,
            Format::F64x4 | Format::F64x3 | Format::F64x2 | Format::F64 => Format::F64,
            Format::U64 => Format::U64,
            Format::U32 => Format::U32,
            Format::U8x3 | Format::U8 => Format::U8,
        }
    }
}
