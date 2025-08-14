use crate::render_graph::resource::memory::MemoryState;

/// Edge describing state change of a buffer resource in the render graph
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub(crate) struct BufferEdge {
    pub(crate) dst_memory: MemoryState,
    pub(crate) dst_queue_family_index: u32,
}
