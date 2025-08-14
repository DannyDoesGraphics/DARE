use thiserror::Error;

#[derive(Debug, Error, Copy, Clone, PartialEq, Eq, Hash)]
pub enum DagalRenderGraphError {
    #[error("Mismatch between requested resource kind and physical resource kind")]
    MismatchResourceKind,
    #[error("Physical resource not found in the render graph")]
    ResourceNotFound,
}
unsafe impl Send for DagalRenderGraphError {}
unsafe impl Sync for DagalRenderGraphError {}
