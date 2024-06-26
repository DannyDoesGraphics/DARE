use std::sync::Arc;

#[derive(Debug)]
pub struct Pipeline<P: dagal::pipelines::traits::Pipeline> {
    handle: P,
    layout: Arc<dagal::pipelines::PipelineLayout>,
}

impl<P: dagal::pipelines::traits::Pipeline> Pipeline<P> {
    pub fn new(handle: P, layout: Arc<dagal::pipelines::PipelineLayout>) -> Self {
        Self {
            handle,
            layout
        }
    }

    pub fn get_pipeline(&self) -> &P {
        &self.handle
    }

    pub fn get_layout(&self) -> &Arc<dagal::pipelines::PipelineLayout> {
        &self.layout
    }
}