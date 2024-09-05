#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct Format {
    element_format: ElementFormat,
    dimension: usize,
}

impl Format {
    pub fn element_size(&self) -> usize {
        self.element_format.size()
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }

    pub fn size(&self) -> usize {
        self.element_format.size() * self.dimension
    }

    pub fn element_format(&self) -> ElementFormat {
        self.element_format
    }

    pub fn new(element_format: ElementFormat, dimension: usize) -> Self {
        Self {
            element_format,
            dimension,
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum ElementFormat {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

impl ElementFormat {
    pub fn size(&self) -> usize {
        match &self {
            ElementFormat::U8 => 1,
            ElementFormat::U16 => 2,
            ElementFormat::U32 => 4,
            ElementFormat::U64 => 8,
            ElementFormat::I8 => 1,
            ElementFormat::I16 => 2,
            ElementFormat::I32 => 4,
            ElementFormat::I64 => 8,
            ElementFormat::F32 => 4,
            ElementFormat::F64 => 8,
        }
    }

    pub fn cast_slice(&self) {}
}