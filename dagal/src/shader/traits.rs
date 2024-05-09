use anyhow::Result;

/// Responsible for compiling the shader
pub trait ShaderCompiler {
	type ShaderType: Shader;
	
	/// Creates a new compiler
	fn new() -> Self;
	
	/// Compiles a shader
	fn compile(content: &str) -> Result<Self::ShaderType>;
}

pub trait Shader {
}