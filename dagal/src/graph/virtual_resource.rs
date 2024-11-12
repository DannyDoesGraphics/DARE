/// An opaque representation of a generic [`dagal::resource::traits::Resource`]
#[derive(PartialEq, Eq, Debug, Hash)]
pub struct VirtualResource {
    pub(crate) id: u32,
    pub(crate) generation: u32,
}
