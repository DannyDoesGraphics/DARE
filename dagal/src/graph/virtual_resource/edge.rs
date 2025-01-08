use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use crate::graph::virtual_resource::{ResourceHandle, ResourceHandleUntyped};

#[derive(PartialEq, Eq, Clone, Debug)]
pub(crate) enum VirtualResourceEdge {
    Read(ResourceHandleUntyped),
    Write(ResourceHandleUntyped),
    ReadWrite(ResourceHandleUntyped),
}
impl Hash for VirtualResourceEdge {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}
impl Deref for VirtualResourceEdge {
    type Target = ResourceHandleUntyped;

    fn deref(&self) -> &Self::Target {
        match self {
            VirtualResourceEdge::Read(a) => a,
            VirtualResourceEdge::Write(a) => a,
            VirtualResourceEdge::ReadWrite(a) => a
        }
    }
}
impl DerefMut for VirtualResourceEdge {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            VirtualResourceEdge::Read(a) => a,
            VirtualResourceEdge::Write(a) => a,
            VirtualResourceEdge::ReadWrite(a) => a,
        }
    }
}
impl VirtualResourceEdge {
    pub fn write(&self) -> bool {
        match self {
            VirtualResourceEdge::Read(_) => false,
            VirtualResourceEdge::Write(_) => true,
            VirtualResourceEdge::ReadWrite(_) => true
        }
    }
}