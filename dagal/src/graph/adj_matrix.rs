/// Defines an adjacency matrix
#[derive(Debug, PartialEq, Eq, Hash, Default)]
pub(crate) struct AdjMatrix<T: Default + Clone> {
    pub(crate) matrix: Vec<Vec<T>>,
}
unsafe impl<T: Default + Clone> Send for AdjMatrix<T> {}
impl<T: Default + Clone> Unpin for AdjMatrix<T> {}

impl<T: Default + Clone> AdjMatrix<T> {
    /// Push a vertex into the back of the adjacency matrix
    pub fn push_vertex(&mut self) {
        let row_length: usize = self.matrix.len() + 1;
        // update existing tows
        let _ = self
            .matrix
            .iter_mut()
            .map(|row| row.resize(row_length, T::default()));
        self.matrix.push(vec![T::default(); row_length]);
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
