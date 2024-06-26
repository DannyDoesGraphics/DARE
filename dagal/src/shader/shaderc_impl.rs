use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use shaderc::{IncludeType, ResolvedInclude};

/// Implementation of [`shaderc`] compiler
pub struct ShaderCCompiler {
    handle: shaderc::Compiler,
}

impl super::traits::ShaderCompiler for ShaderCCompiler {
    fn new() -> Self {
        Self {
            handle: shaderc::Compiler::new().unwrap(),
        }
    }

    fn compile_file(
        &self,
        in_path: PathBuf,
        out_path: PathBuf,
        shader_kind: super::ShaderKind,
    ) -> Result<()> {
        if !super::is_file_newer(in_path.clone(), out_path.clone())? {
            Ok(())
        } else {
            let in_content = std::fs::read_to_string(in_path.clone())?;
            let output = self.compile(
                in_content.as_str(),
                shader_kind,
                in_path.file_name().unwrap().to_str().unwrap(),
            )?;
            let output: Vec<u8> = output.iter().flat_map(|data| data.to_le_bytes()).collect();
            std::fs::write(out_path, output.as_slice())?;
            Ok(())
        }
    }

    fn compile(
        &self,
        content: &str,
        shader_kind: super::ShaderKind,
        shader_name: &str,
    ) -> Result<Vec<u32>> {
        let options = shaderc::CompileOptions::new();
        if options.is_none() {
            return Err(anyhow::Error::from(crate::DagalError::ShadercError));
        }
        let mut options = options.unwrap();
        options.add_macro_definition("EP", Some("main"));
        options.set_warnings_as_errors();
        let include_context = Arc::new(Mutex::new(super::glsl_preprocessor::IncludeContext::new()));

        options.set_include_callback({
            let include_context = include_context.clone();
            move |requested_path, include_type, including_path, _| {
                let source_path = PathBuf::from(including_path).canonicalize().unwrap();
                let include_path = match include_type {
                    IncludeType::Relative => {
                        let path = source_path.parent().unwrap();
                        let requested_path = requested_path.trim_start_matches("./");
                        let path = path.join(requested_path);
                        path.canonicalize().unwrap_or_else(|_| panic!("Cannot find path for {:?}", path))
                    }
                    IncludeType::Standard => {
                        if requested_path.starts_with("dagal/") {
                            let requested_path_str = requested_path.trim_start_matches("dagal/");
                            PathBuf::from("dagal/shaders/includes").join(requested_path_str)
                        } else {
                            PathBuf::from(requested_path)
                        }
                    }
                };
                //let include_path = include_path.canonicalize().unwrap();

                let mut guard = include_context.lock().unwrap();
                let res = guard
                    .resolve_include(source_path, include_path)
                    .map_err(|err| err.to_string())?;
                Ok(ResolvedInclude {
                    resolved_name: res.resolved_name,
                    content: res.content,
                })
            }
        });

        let output = self.handle.compile_into_spirv(
            content,
            shaderc::ShaderKind::from(shader_kind),
            shader_name,
            "main",
            Some(&options),
        )?;

        Ok(output.as_binary().to_vec())
    }
}

impl From<super::ShaderKind> for shaderc::ShaderKind {
    fn from(value: super::ShaderKind) -> Self {
        match value {
            super::ShaderKind::Compute => shaderc::ShaderKind::Compute,
            super::ShaderKind::Geometry => shaderc::ShaderKind::Geometry,
            super::ShaderKind::Vertex => shaderc::ShaderKind::Vertex,
            super::ShaderKind::Fragment => shaderc::ShaderKind::Fragment,
        }
    }
}
