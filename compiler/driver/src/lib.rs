//! Compiler driver library: pipeline orchestration entry points.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` sections 2.7-2.8.

use std::path::Path;

/// A single compiler diagnostic.
pub struct Diagnostic {
    pub message: String,
}

/// Compile one source file through the full pipeline.
///
/// Milestone M0: no stage is implemented yet, so every input produces a
/// formal "unsupported" diagnostic (see `docs/ROADMAP.md`). This is a
/// user-facing error path, not a code placeholder: the diagnostic is
/// removed as milestones land real stages.
pub fn compile_file(path: &Path) -> Result<(), Vec<Diagnostic>> {
    std::fs::read_to_string(path).map_err(|e| {
        vec![Diagnostic {
            message: format!("error: cannot read {}: {e}", path.display()),
        }]
    })?;
    Err(vec![Diagnostic {
        message: "error: no compilation stage is available yet \
                  (milestone M0: harness only, see docs/ROADMAP.md)"
            .to_string(),
    }])
}
