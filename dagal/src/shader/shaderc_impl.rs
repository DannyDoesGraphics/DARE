use anyhow::Result;

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
        in_path: std::path::PathBuf,
        out_path: std::path::PathBuf,
        shader_kind: super::ShaderKind,
    ) -> Result<()> {
        if !super::traits::is_file_newer(in_path.clone(), out_path.clone())? {
            Ok(())
        } else {
            let in_content = std::fs::read_to_string(in_path.clone())?;
            let output = self.compile(
                in_content.as_str(),
                shader_kind,
                in_path.file_name().unwrap().to_str().unwrap(),
            )?;
            std::fs::write(out_path, output.as_slice())?;
            Ok(())
        }
    }

    fn compile(
        &self,
        content: &str,
        shader_kind: super::ShaderKind,
        shader_name: &str,
    ) -> Result<Vec<u8>> {
        let options = shaderc::CompileOptions::new();
        if options.is_none() {
            return Err(anyhow::Error::from(crate::DagalError::ShadercError));
        }
        let mut options = options.unwrap();
        options.add_macro_definition("EP", Some("main"));

        let output = self.handle.compile_into_spirv(
            content,
            shaderc::ShaderKind::from(shader_kind),
            shader_name,
            "main",
            Some(&options),
        )?;

        Ok(output.as_binary_u8().to_vec())
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
