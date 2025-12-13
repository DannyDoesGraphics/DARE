pub enum StreamState<Handle> {
    Vacant,
    Loading,
    Resident(Handle),
    Failed,
}
