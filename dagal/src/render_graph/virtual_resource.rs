use std::hash::Hash;

/// A virtual resource handle is a virtual opaque handle to a resource
/// Which may or may not be instantiated
#[derive(Debug, Clone, Eq)]
pub struct VirtualResource {
    /// Upper 48 bits are virtual resource id, bottom 16 bits are resource version
    pub(crate) data: u64,
    pub(crate) kind: std::any::TypeId,
}
impl Hash for VirtualResource {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}
impl PartialEq for VirtualResource {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl VirtualResource {
    pub fn new<T: 'static>(id: u64, version: u16) -> Self {
        assert!(id < (1 << 48), "Virtual resource id must be less than 2^48");

        Self {
            data: (id << 16) | (version as u64),
            kind: std::any::TypeId::of::<T>(),
        }
    }

    pub fn id(&self) -> u64 {
        self.data >> 16
    }

    pub fn version(&self) -> u16 {
        (self.data & 0xFFFF) as u16
    }

    pub fn kind(&self) -> std::any::TypeId {
        self.kind
    }

    pub(crate) fn set_version(&mut self, version: u16) {
        self.data = (self.data & !0xFFFF) | (version as u64);
    }

    pub fn set_id(&mut self, id: u64) {
        assert!(id < (1 << 48), "Virtual resource id must be less than 2^48");
        self.data = (id << 16) | (self.data & 0xFFFF);
    }
}
