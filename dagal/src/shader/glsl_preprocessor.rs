use anyhow::Result;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;

/// Handles pre-processing GLSL shaders

/// Result
pub struct ResolvedInclude {
    /// Name of the resolved include file (Should be absolute)
    pub resolved_name: String,

    /// Contents of the resolve include file
    pub content: String,
}

/// Handles #include directives into glsl
#[derive(Debug, Clone, Default)]
pub struct IncludeContext {
    included_files: HashSet<PathBuf>,
    include_stack: VecDeque<PathBuf>,
}

impl IncludeContext {
    pub fn new() -> Self {
        Self {
            included_files: HashSet::new(),
            include_stack: VecDeque::new(),
        }
    }

    pub fn resolve_include(
        &mut self,
        source_path: PathBuf,
        include_path: PathBuf,
    ) -> Result<ResolvedInclude> {
        if self.include_stack.contains(&include_path) {
            return Err(anyhow::anyhow!(format!(
                "Invalid #include usage found in {:?}. Trying to include {:?}",
                &source_path, &include_path
            )));
        } else if self.included_files.contains(&include_path) {
            // double include
            return Ok(ResolvedInclude {
                resolved_name: include_path.to_string_lossy().to_string(),
                content: String::new(),
            });
        }
        self.include_stack.push_back(include_path.clone());

        let res = if include_path.exists() {
            let content = fs::read_to_string(&include_path)?;
            self.included_files.insert(include_path.clone());
            Ok(ResolvedInclude {
                resolved_name: include_path.to_string_lossy().to_string(),
                content,
            })
        } else {
            Err(anyhow::anyhow!(format!(
                "Tried to #include for {:?} in {:?}. Does not exist.",
                &include_path, &source_path
            )))
        };
        self.include_stack.pop_back();
        res
    }
}
