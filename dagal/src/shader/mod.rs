/// Provides traits for whatever shader provider you wish to use to compile & process your shaders
pub mod traits;

use ash::vk;
use std::env;
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

impl From<vk::ShaderStageFlags> for ShaderKind {
    fn from(value: vk::ShaderStageFlags) -> Self {
        match value {
            vk::ShaderStageFlags::VERTEX => ShaderKind::Vertex,
            vk::ShaderStageFlags::COMPUTE => ShaderKind::Compute,
            vk::ShaderStageFlags::FRAGMENT => ShaderKind::Fragment,
            vk::ShaderStageFlags::GEOMETRY => ShaderKind::Geometry,
            _ => unimplemented!(),
        }
    }
}

/// Checks if path_in is newer than path_out
pub(crate) fn is_file_newer(
    path_in: std::path::PathBuf,
    path_out: std::path::PathBuf,
) -> anyhow::Result<bool> {
    println!("Current directory: {:?}", env::current_dir().unwrap());
    let metadata_in = std::fs::metadata(path_in)?.modified()?;
    match std::fs::metadata(path_out) {
        Ok(metadata_out) => Ok(metadata_in > metadata_out.modified()?),
        Err(e) => {
            println!("Encountered {e}");
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(true)
            } else {
                Err(anyhow::Error::from(e))
            }
        }
    }
}
