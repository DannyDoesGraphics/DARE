#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct Format {
    element_format: ElementFormat,
    dimension: usize,
}

impl Format {
    pub fn new(element_format: ElementFormat, dimension: usize) -> Self {
        Self {
            element_format,
            dimension,
        }
    }

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
}

impl From<gltf::json::accessor::ComponentType> for ElementFormat {
    fn from(value: gltf::json::accessor::ComponentType) -> Self {
        use gltf::json::accessor::ComponentType;
        match value {
            ComponentType::I8 => Self::I8,
            ComponentType::U8 => Self::U8,
            ComponentType::I16 => Self::I16,
            ComponentType::U16 => Self::U16,
            ComponentType::U32 => Self::U32,
            ComponentType::F32 => Self::F32,
        }
    }
}
