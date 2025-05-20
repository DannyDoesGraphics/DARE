use crate::graph::RenderGraph;

pub struct RasterPass<'a> {
    name: String,
    graph: &'a mut RenderGraph,
    render_targets: Vec<()>,
}
