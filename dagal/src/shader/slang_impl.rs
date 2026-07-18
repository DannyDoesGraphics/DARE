use std::path::PathBuf;

use anyhow::Result;

pub struct SlangCompiler {
    global_session: shader_slang::GlobalSession,
}

impl SlangCompiler {
    fn to_stage(shader_kind: super::ShaderKind) -> shader_slang::Stage {
        match shader_kind {
            super::ShaderKind::Vertex => shader_slang::Stage::Vertex,
            super::ShaderKind::Fragment => shader_slang::Stage::Fragment,
            super::ShaderKind::Compute => shader_slang::Stage::Compute,
            super::ShaderKind::Geometry => shader_slang::Stage::Geometry,
        }
    }

    fn create_session(&self, search_paths: &[*const i8]) -> Result<shader_slang::Session> {
        let session_options = shader_slang::CompilerOptions::default()
            .glsl_force_scalar_layout(true)
            .emit_spirv_directly(true)
            .capability(self.global_session.find_capability("GL_EXT_buffer_reference"));

        let target_desc = shader_slang::TargetDesc::default()
            .format(shader_slang::CompileTarget::Spirv)
            .profile(self.global_session.find_profile("glsl_460"));

        let targets = [target_desc];
        let session_desc = shader_slang::SessionDesc::default()
            .targets(&targets)
            .search_paths(search_paths)
            .options(&session_options);

        self.global_session
            .create_session(&session_desc)
            .ok_or_else(|| anyhow::anyhow!("Failed to create Slang session"))
    }

    fn compile_stage(
        session: &shader_slang::Session,
        module: shader_slang::Module,
        target_stage: shader_slang::Stage,
    ) -> Result<Vec<u32>> {
        for entry_point in module.entry_points() {
            let program = session
                .create_composite_component_type(&[module.clone().into(), entry_point.into()])
                .map_err(|err| anyhow::anyhow!("{err:?}"))?;
            let linked = program.link().map_err(|err| anyhow::anyhow!("{err:?}"))?;
            let stage = linked
                .layout(0)
                .map_err(|err| anyhow::anyhow!("{err:?}"))?
                .entry_point_by_index(0)
                .ok_or_else(|| anyhow::anyhow!("Linked program has no entry point"))?
                .stage();

            if stage == target_stage {
                let bytecode = linked
                    .entry_point_code(0, 0)
                    .map_err(|err| anyhow::anyhow!("{err:?}"))?;
                return Ok(bytecode
                    .as_slice()
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect());
            }
        }

        Err(anyhow::anyhow!(
            "Module '{}' has no entry point for stage {target_stage:?}",
            module.name()
        ))
    }
}

impl super::traits::ShaderCompiler for SlangCompiler {
    fn new() -> Self {
        Self {
            global_session: shader_slang::GlobalSession::new()
                .expect("Failed to create Slang global session"),
        }
    }

    fn compile_file(
        &self,
        in_path: PathBuf,
        out_path: PathBuf,
        shader_kind: super::ShaderKind,
    ) -> Result<()> {
        if !super::is_file_newer(in_path.clone(), out_path.clone())? {
            return Ok(());
        }

        let search_dir = in_path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let module_name = in_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid Slang file name: {in_path:?}"))?;

        let search_path = std::ffi::CString::new(search_dir.to_string_lossy().as_ref())?;
        let session = self.create_session(&[search_path.as_ptr()])?;
        let module = session
            .load_module(module_name)
            .map_err(|err| anyhow::anyhow!("{err:?}"))?;
        let output = Self::compile_stage(&session, module, Self::to_stage(shader_kind))?;

        let output: Vec<u8> = output.iter().flat_map(|data| data.to_le_bytes()).collect();
        std::fs::write(out_path, output.as_slice())?;
        Ok(())
    }

    fn compile(
        &self,
        content: &str,
        shader_kind: super::ShaderKind,
        shader_name: &str,
    ) -> Result<Vec<u32>> {
        let session = self.create_session(&[])?;
        let module = session
            .load_module_from_source_string(shader_name, shader_name, content)
            .map_err(|err| anyhow::anyhow!("{err:?}"))?;
        Self::compile_stage(&session, module, Self::to_stage(shader_kind))
    }
}
