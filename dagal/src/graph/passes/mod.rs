mod raster_pass;

use crate::graph::TaskResourceId;
use std::fmt::Debug;

pub trait GraphPass: Debug {
    /// Input of resources used by the pass
    fn input_resources(&self) -> &[TaskResourceId];
    /// Output of resources used by the pass
    fn output_resources(&self) -> &[TaskResourceId];
}
