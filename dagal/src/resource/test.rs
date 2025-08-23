use crate::device::LogicalDevice;
use crate::resource::traits::Resource;
use crate::traits::AsRaw;
/// Defines a test resource used for testing purposes only
use std::hash::{Hash, Hasher};

#[derive(Clone)]
pub struct TestResource {
    id: u64,
}

impl Hash for TestResource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl AsRaw for TestResource {
    type RawType = u64;

    unsafe fn as_raw(&self) -> &Self::RawType {
        &self.id
    }

    unsafe fn as_raw_mut(&mut self) -> &mut Self::RawType {
        &mut self.id
    }

    unsafe fn raw(self) -> Self::RawType {
        self.id
    }
}

impl Resource for TestResource {
    type CreateInfo<'a> = u64;

    fn new(create_info: Self::CreateInfo<'_>) -> Result<Self, crate::DagalError>
    where
        Self: Sized,
    {
        Ok(Self { id: create_info })
    }

    fn get_device(&self) -> &LogicalDevice {
        panic!("Test resource does not support physical devices due to test constraints")
    }
}
