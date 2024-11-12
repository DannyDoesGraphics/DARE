/// Defines an adjacency matrix
#[derive(Debug, PartialEq, Eq, Hash, Default)]
pub(crate) struct AdjMatrix {
    pub(crate) matrix: Vec<Vec<bool>>,
}
unsafe impl Send for AdjMatrix {}
impl Unpin for AdjMatrix {}

impl AdjMatrix {
    /// Push a vertex into the back of the adjacency matrix
    pub fn push_vertex(&mut self) {
        let row_length: usize = self.matrix.len() + 1;
        // update existing tows
        let _ = self
            .matrix
            .iter_mut()
            .map(|row| row.resize(row_length, false));
        self.matrix.push(vec![false; row_length]);
    }

    /// Removes a vertex in the adjacency matrix
    pub fn remove_vertex_at(&mut self, index: usize) {
        // remove row
        self.matrix.remove(index);
        // update remaining rows
        for row in self.matrix.iter_mut() {
            row.remove(index);
        }
    }
}
