use crate::shader::ShaderKind;
use anyhow::Result;
use std::env;

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
    fn compile(
        &self,
        content: &str,
        shader_kind: ShaderKind,
        shader_name: &str,
    ) -> Result<Vec<u32>>;
}
