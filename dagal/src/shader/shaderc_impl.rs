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
        if !super::is_file_newer(in_path.clone(), out_path.clone())? {
            Ok(())
        } else {
            let in_content = std::fs::read_to_string(in_path.clone())?;
            let output = self.compile(
                in_content.as_str(),
                shader_kind,
                in_path.file_name().unwrap().to_str().unwrap(),
            )?;
            let output: Vec<u8> = output
                .iter()
                .flat_map(|data| data.to_le_bytes())
                .collect();
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
