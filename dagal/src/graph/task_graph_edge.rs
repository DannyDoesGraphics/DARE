use ash::vk;

/// Defines edges for task graphs

pub enum ResourceTransitions {
    Buffer {
        src_queue: u32,
        dst_queue: u32,
    },
    Image {
        src_queue: u32,
        dst_queue: u32,
        src_layout: vk::ImageLayout,
        dst_layout: vk::ImageLayout,
    },
}

#[derive(Debug, Clone)]
pub enum Edge {
    MutEdge(),
    Edge(),
    NoEdge,
}
impl Default for Edge {
    fn default() -> Self {
        Self::NoEdge
    }
}
