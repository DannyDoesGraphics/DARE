use bevy_ecs::prelude::*;

#[derive(Debug, Resource)]
pub struct ComputeCullContext {
    pub pipeline: dagal::pipelines::ComputePipeline,
    pub pipeline_layout: dagal::pipelines::PipelineLayout,
    pub descriptor_set_layout: dagal::descriptor::DescriptorSetLayout,
}
