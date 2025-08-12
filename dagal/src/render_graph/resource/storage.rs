use crate::virtual_resource::VirtualResource;
use std::collections::HashMap;
use std::fmt::Debug;

/// Holds physical resources for any virtual that exist in the render graph
#[derive(Debug)]
pub struct PhysicalResourceStorage<'a> {
    /// Maps virtual resources to their physical counterparts
    pub(crate) resources: HashMap<VirtualResource, Box<dyn 'a + Send + Sync + Debug>>,

    pub(crate) virtual_resource_metadata: HashMap<VirtualResource, Box<dyn 'a + Debug>>,
}

impl<'a> PhysicalResourceStorage<'a> {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            virtual_resource_metadata: HashMap::new(),
        }
    }
}
