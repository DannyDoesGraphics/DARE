/// Provides traits for whatever shader provider you wish to use to compile & process your shaders
pub mod traits;
pub use traits::*;
pub mod shader;
pub use shader::Shader;

#[cfg(feature = "shaderc")]
pub mod shaderc_impl;
#[cfg(feature = "shaderc")]
pub use shaderc_impl::*;

#[derive(Copy, Debug, Clone, PartialOrd, PartialEq)]
pub enum ShaderKind {
    Compute,
    Geometry,
    Vertex,
    Fragment,
}
