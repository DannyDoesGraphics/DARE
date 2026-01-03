
/// A unit stream is a stream that packages another incoming stream of bytes into units
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct UnitStream {}

impl Default for UnitStream {
    fn default() -> Self {
        UnitStream {}
    }
}

