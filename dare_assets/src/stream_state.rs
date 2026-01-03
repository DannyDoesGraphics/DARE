#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum StreamState {
    Vacant = 0,
    Loading = 1,
    Resident = 2,
    Evicted = 3,
    Failed = 4,
}
