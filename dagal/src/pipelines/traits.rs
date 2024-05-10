use crate::traits::Destructible;
use anyhow::Result;

pub trait PipelineBuilder: Default {
    type BuildTo: Pipeline;

    fn build() -> Result<Self::BuildTo>;
}

pub trait Pipeline: Destructible {}
