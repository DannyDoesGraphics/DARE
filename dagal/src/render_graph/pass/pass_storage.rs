
use std::collections::HashMap;
use ash::vk;

/// Extremely primitive version of a PSO manager, and we should rely on one later
#[derive(Debug)]
pub struct PassStorage {
    pub id: u32,
    pub passes: HashMap<super::PassId, vk::Pipeline>,
}

impl PassStorage {
    pub fn new() -> Self {
        Self {
            id: 0,
            passes: HashMap::new(),
        }
    }

    pub fn insert_pass(&mut self, pipeline: vk::Pipeline) -> super::PassId {
        let id = self.id;
        self.id += 1;
        let pass_id = super::PassId(id);
        self.passes.insert(pass_id, pipeline);
        pass_id
    }

    pub fn detach_pass(&mut self, pass_id: &super::PassId) -> Option<vk::Pipeline> {
        self.passes.remove(pass_id)
    }

    pub fn get_pass(&self, pass_id: &super::PassId) -> Option<&vk::Pipeline> {
        self.passes.get(pass_id)
    }
}