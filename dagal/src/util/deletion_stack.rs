use std::collections::HashMap;
use crate::descriptor::GPUResourceTable;
use crate::traits::Destructible;
use crate::util::free_list_allocator::Handle;

/// A stack which is used to delete objects in order
#[derive(Default)]
pub struct DeletionStack<'a> {
    stack: Vec<Box<dyn FnOnce() + 'a>>,
}

impl<'a> DeletionStack<'a> {
    /// Adds item onto the stack
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn push<T>(&mut self, func: T)
    where
        T: FnOnce() + 'a,
    {
        self.stack.push(Box::new(func));
    }

    pub fn push_resource<T: Clone + Destructible + 'a>(&mut self, resource: &T) {
        let mut resource_clone: T = resource.clone();
        self.push(move || {
            resource_clone.destroy();
        });
    }

    pub fn push_resources<T: Clone + Destructible + 'a>(&mut self, resources: &[T]) {
        let mut resources = Vec::from(resources);
        self.push(move || {
            while let Some(mut resource) = resources.pop() {
                resource.destroy();
            }
        });
    }

    pub fn flush(&mut self) {
        while let Some(element) = self.stack.pop() {
            element();
        }
    }
}
