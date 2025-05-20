use petgraph::prelude::*;
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};

pub trait TaskResource: Debug + PartialEq + Eq + Hash {}

/// [`TaskResourceId`] is used as **edges** in a task graph
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TaskResourceId {
    pub(crate) uid: u64,
    pub(crate) generation: u64,
}
impl TaskResourceId {
    /// Make a new task resource id
    pub fn new<R: TaskResource>(resource: &R) -> Self {
        let mut hash = DefaultHasher::new();
        resource.hash(&mut hash);
        Self {
            uid: hash.finish(),
            generation: 0,
        }
    }
}

impl Hash for TaskResourceId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uid.hash(state);
        self.generation.hash(state);
    }
}

pub enum PassType {
    Compute {
        pipeline_id: usize,
        dispatch: [u32; 3],
    },
    Raster {
        pipeline_id: usize,
        dispatch: [u32; 3],
    },
    Custom(Box<dyn Fn()>),
}

/// A task node
#[derive(Debug)]
pub struct TaskNode {
    /// Name of the pass
    pub(crate) name: String,
    /// Input resources
    pub(crate) inputs: Vec<TaskResourceId>,
    /// Output resources
    pub(crate) outputs: Vec<TaskResourceId>,
}

pub struct RenderGraph {
    nodes: Vec<TaskNode>,
    /// For each node, list the dependent nodes
    dependencies: Vec<Vec<usize>>,
    /// In-degree counts for Kahn's algorithm
    in_degree: Vec<usize>,
    next_resource_index: usize,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            dependencies: Vec::new(),
            in_degree: Vec::new(),
            next_resource_index: 0,
        }
    }
}
