use petgraph::prelude::*;
use std::fmt::Debug;
use std::hash::{DefaultHasher, Hash, Hasher};

pub trait TaskResource: Debug + PartialEq + Eq + Hash {}

/// [`TaskResourceId`] is used as **edges** in a task graph
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TaskResourceId {
    pub(crate) uid: u64,
    pub(crate) gen: u64,
}
impl TaskResourceId {
    /// Make a new task resource id
    pub fn new<R: TaskResource>(resource: &R) -> Self {
        let mut hash = DefaultHasher::new();
        resource.hash(&mut hash);
        Self {
            uid: hash.finish(),
            gen: 0,
        }
    }
}
impl Hash for TaskResourceId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uid.hash(state);
        self.gen.hash(state);
    }
}

/// A barrier that uses underlying [`TaskResourceId`]
#[derive(Debug)]
pub struct TaskBarrier {
    pub(crate) resource_id: TaskResourceId,
}

/// A task node
#[derive(Debug)]
pub struct Task {
    pub(crate) inputs: Vec<TaskResourceId>,
    pub(crate) outputs: Vec<TaskResourceId>,
}

#[derive(Debug)]
pub enum TaskNode {
    Task(Task),
    Barrier(TaskBarrier),
}

pub struct TaskGraph {
    graph: DiGraph<TaskNode, TaskResourceId>,
}
