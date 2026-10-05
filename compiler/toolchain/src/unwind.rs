use std::path::{Path, PathBuf};

use scoop_lir::TargetProfileId;

use crate::ToolchainError;

pub fn selected_unwind_prefix(
    target: TargetProfileId,
    sysroot: Option<&Path>,
    explicit: Option<&Path>,
) -> PathBuf {
    explicit.map(Path::to_owned).unwrap_or_else(|| {
        sysroot
            .map(Path::to_owned)
            .unwrap_or_else(|| crate::configured_sysroot_root(None))
            .join("native")
            .join(target.canonical_triple())
            .join("unwind")
    })
}

pub fn runtime_unwind_include(
    target: TargetProfileId,
    prefix: Option<&Path>,
) -> Result<Option<PathBuf>, ToolchainError> {
    if target == TargetProfileId::DarwinAarch64 {
        if prefix.is_some() {
            return Err(ToolchainError(
                "Darwin runtime uses the SDK unwind headers".into(),
            ));
        }
        return Ok(None);
    }
    let prefix = selected_unwind_prefix(target, None, prefix);
    let include = prefix.join("include");
    if !include.join("unwind.h").is_file() {
        return Err(ToolchainError(format!(
            "missing LLVM unwind headers at {}; provide --unwind-prefix or run scripts/build_llvm_unwind.py for {}",
            include.display(),
            target.canonical_triple()
        )));
    }
    // Keep this locator's symlinks: the dependency cache must observe changes
    // to the selected header alias, not only to its previous real path.
    std::path::absolute(include)
        .map(Some)
        .map_err(|error| ToolchainError(error.to_string()))
}
