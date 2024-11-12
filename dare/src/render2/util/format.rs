use futures_core::stream::BoxStream;

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

pub fn handle_cast_stream(
    stream: BoxStream<Vec<u8>>,
    source_format: Format,
    target_format: Format,
    in_chunk_size: usize,
) -> BoxStream<Vec<u8>> {
    // Helper macro for calling CastStream with appropriate types
    use futures::stream::StreamExt;
    macro_rules! cast_stream {
        ($stream:expr, $src:ty, $dst:ty) => {
            crate::prelude::asset2::loaders::CastStream::<_, $src, $dst>::new(
                $stream,
                in_chunk_size,
                source_format.dimension(),
                target_format.dimension(),
            )
            .boxed()
        };
    }

    // If formats are identical (including dimension), return the original stream without any casting.
    if source_format == target_format {
        return stream;
    }

    // Panic if attempting a downcast in element size
    if source_format.element_size() > target_format.element_size() {
        panic!(
            "Cannot downcast from {:?} -> {:?}",
            source_format, target_format
        );
    }

    use crate::prelude::render::util::ElementFormat;

    // Match on each possible upcast case, including different element types and dimensions
    let stream = match (
        source_format.element_format(),
        target_format.element_format(),
    ) {
        // Same type but different dimensions
        (ElementFormat::U8, ElementFormat::U8) => cast_stream!(stream, u8, u8),
        (ElementFormat::U16, ElementFormat::U16) => cast_stream!(stream, u16, u16),
        (ElementFormat::U32, ElementFormat::U32) => cast_stream!(stream, u32, u32),
        (ElementFormat::U64, ElementFormat::U64) => cast_stream!(stream, u64, u64),
        (ElementFormat::I8, ElementFormat::I8) => cast_stream!(stream, i8, i8),
        (ElementFormat::I16, ElementFormat::I16) => cast_stream!(stream, i16, i16),
        (ElementFormat::I32, ElementFormat::I32) => cast_stream!(stream, i32, i32),
        (ElementFormat::I64, ElementFormat::I64) => cast_stream!(stream, i64, i64),
        (ElementFormat::F32, ElementFormat::F32) => cast_stream!(stream, f32, f32),
        (ElementFormat::F64, ElementFormat::F64) => cast_stream!(stream, f64, f64),

        // Unsigned to Signed/Unsigned Integer Upcasts
        (ElementFormat::U8, ElementFormat::U16) => cast_stream!(stream, u8, u16),
        (ElementFormat::U8, ElementFormat::U32) => cast_stream!(stream, u8, u32),
        (ElementFormat::U8, ElementFormat::U64) => cast_stream!(stream, u8, u64),
        (ElementFormat::U8, ElementFormat::I16) => cast_stream!(stream, u8, i16),
        (ElementFormat::U8, ElementFormat::I32) => cast_stream!(stream, u8, i32),
        (ElementFormat::U8, ElementFormat::I64) => cast_stream!(stream, u8, i64),
        (ElementFormat::U16, ElementFormat::U32) => cast_stream!(stream, u16, u32),
        (ElementFormat::U16, ElementFormat::U64) => cast_stream!(stream, u16, u64),
        (ElementFormat::U16, ElementFormat::I32) => cast_stream!(stream, u16, i32),
        (ElementFormat::U16, ElementFormat::I64) => cast_stream!(stream, u16, i64),
        (ElementFormat::U32, ElementFormat::U64) => cast_stream!(stream, u32, u64),
        (ElementFormat::U32, ElementFormat::I64) => cast_stream!(stream, u32, i64),

        // Signed to Signed/Unsigned Integer Upcasts
        (ElementFormat::I8, ElementFormat::I16) => cast_stream!(stream, i8, i16),
        (ElementFormat::I8, ElementFormat::I32) => cast_stream!(stream, i8, i32),
        (ElementFormat::I8, ElementFormat::I64) => cast_stream!(stream, i8, i64),
        (ElementFormat::I8, ElementFormat::U16) => cast_stream!(stream, i8, u16),
        (ElementFormat::I8, ElementFormat::U32) => cast_stream!(stream, i8, u32),
        (ElementFormat::I8, ElementFormat::U64) => cast_stream!(stream, i8, u64),
        (ElementFormat::I16, ElementFormat::I32) => cast_stream!(stream, i16, i32),
        (ElementFormat::I16, ElementFormat::I64) => cast_stream!(stream, i16, i64),
        (ElementFormat::I16, ElementFormat::U32) => cast_stream!(stream, i16, u32),
        (ElementFormat::I16, ElementFormat::U64) => cast_stream!(stream, i16, u64),
        (ElementFormat::I32, ElementFormat::I64) => cast_stream!(stream, i32, i64),
        (ElementFormat::I32, ElementFormat::U64) => cast_stream!(stream, i32, u64),

        // Floating Point Upcasts
        (ElementFormat::F32, ElementFormat::F64) => cast_stream!(stream, f32, f64),

        // Unsupported cases will trigger a panic at runtime
        _ => unreachable!(
            "Unsupported cast from {:?} to {:?}",
            source_format.element_format(),
            target_format.element_format()
        ),
    };

    stream
}
