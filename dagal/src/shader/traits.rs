use crate::shader::ShaderKind;
use anyhow::Result;

/// Responsible for compiling the shader
pub trait ShaderCompiler {
    /// Creates a new compiler
    fn new() -> Self;

    /// Compiles a file and outputs it.
    ///
    /// It is expected that it will not recompile if the file has not been changed compared to the
    /// out location
    fn compile_file(
        &self,
        in_path: std::path::PathBuf,
        out_path: std::path::PathBuf,
        shader_kind: ShaderKind,
    ) -> Result<()>;

    /// Compiles a shader from given string content and outputs the spir-v contents
    fn compile(&self, content: &str, shader_kind: ShaderKind, shader_name: &str)
        -> Result<Vec<u8>>;
}

/// Checks if path_in is newer than path_out
pub(super) fn is_file_newer(path_in: std::path::PathBuf, path_out: std::path::PathBuf) -> Result<bool> {
    let metadata_in = std::fs::metadata(path_in)?.modified()?;
    match std::fs::metadata(path_out) {
        Ok(metadata_out) => Ok(metadata_in > metadata_out.modified()?),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(true)
            } else {
                Err(anyhow::Error::from(e))
            }
        }
    }
}
