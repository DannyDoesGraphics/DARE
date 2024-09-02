use thiserror::Error;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Error)]
pub enum ContainerErrors {
    #[error("Expected a valid slot, got null")]
    NonexistentSlot,
}
