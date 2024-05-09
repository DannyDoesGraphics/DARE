/// Provides traits for whatever shader provider you wish to use to compile & process your shaders

pub mod traits;
pub use traits::*;

#[cfg(feature = "shaderc")]
pub mod shaderc_impl;
#[cfg(feature = "shaderc")]
pub use shaderc_impl::*;