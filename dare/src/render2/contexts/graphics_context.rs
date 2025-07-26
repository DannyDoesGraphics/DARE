use bevy_ecs::prelude as becs;
use dagal::pipelines::{GraphicsPipeline, PipelineLayout};

/// Context that manages graphics pipeline resources
#[derive(Debug, becs::Resource)]
pub struct GraphicsContext {
    pub graphics_pipeline: GraphicsPipeline,
    pub graphics_layout: PipelineLayout,
}

impl GraphicsContext {
    pub fn new(graphics_pipeline: GraphicsPipeline, graphics_layout: PipelineLayout) -> Self {
        Self {
            graphics_pipeline,
            graphics_layout,
        }
    }
}

impl Drop for GraphicsContext {
    fn drop(&mut self) {
        tracing::trace!("Dropped GraphicsContext");
    }
}
