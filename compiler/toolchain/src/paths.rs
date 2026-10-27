use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

pub fn configured_cache_root(explicit: Option<PathBuf>) -> Result<PathBuf, crate::ToolchainError> {
    explicit
        .or_else(|| std::env::var_os("SCOOP_CACHE_DIR").map(PathBuf::from))
        .or_else(default_cache)
        .ok_or_else(|| {
            crate::ToolchainError(
                "no usable platform cache directory; provide --cache-dir or SCOOP_CACHE_DIR".into(),
            )
        })
}

fn default_cache() -> Option<PathBuf> {
    if cfg!(windows) {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|path| path.join("Scoop/Cache"));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    if cfg!(target_os = "linux") {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|path| path.join("scoop"))
            .or_else(|| home.map(|path| path.join(".cache/scoop")))
    } else {
        home.map(|path| path.join("Library/Caches/Scoop"))
    }
}

/// Build-time source layout for repository tests and maintenance tools.
pub fn development_workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("toolchain crate lives under compiler in the workspace")
        .to_owned()
}

pub fn configured_sysroot_root(
    explicit: Option<PathBuf>,
) -> Result<PathBuf, crate::ToolchainError> {
    if let Some(root) = explicit.or_else(|| std::env::var_os("SCOOP_SYSROOT").map(PathBuf::from)) {
        return Ok(root);
    }
    let executable = installed_executable()?;
    executable
        .parent()
        .and_then(Path::parent)
        .map(Path::to_owned)
        .ok_or_else(|| {
            crate::ToolchainError(format!(
                "cannot locate sysroot from {}",
                executable.display()
            ))
        })
}

pub fn installed_executable() -> Result<PathBuf, crate::ToolchainError> {
    std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|error| crate::ToolchainError(format!("cannot locate executable: {error}")))
}

pub fn host_executable_name(name: impl AsRef<OsStr>) -> OsString {
    let mut name = name.as_ref().to_owned();
    name.push(std::env::consts::EXE_SUFFIX);
    name
}

pub fn installed_host_tool(name: &str) -> Result<PathBuf, crate::ToolchainError> {
    installed_executable().map(|path| path.with_file_name(host_executable_name(name)))
}

pub fn development_runtime_root() -> PathBuf {
    development_workspace_root().join("runtime")
}

pub fn target_library_root(sysroot: &Path, target: scoop_lir::LirTargetProfile) -> PathBuf {
    sysroot
        .join("lib")
        .join(target.contract().canonical_triple())
}

pub fn artifact_candidate_path(
    root: &Path,
    coordinate: &scoop_identity::ConeCoordinate,
) -> PathBuf {
    root.join(coordinate.group())
        .join(coordinate.name())
        .join(coordinate.version())
        .join("cone.slib")
}

pub fn sysroot_library_path(
    sysroot: &Path,
    target: scoop_lir::LirTargetProfile,
    coordinate: &scoop_identity::ConeCoordinate,
) -> PathBuf {
    target_library_root(sysroot, target).join(format!("{}.slib", coordinate.name()))
}

pub fn default_runtime_index(
    sysroot: &Path,
    target: scoop_lir::LirTargetProfile,
    enable_gc_stress: bool,
    enable_startup_stats: bool,
) -> PathBuf {
    target_library_root(sysroot, target)
        .join(match (enable_gc_stress, enable_startup_stats) {
            (false, false) => "runtime",
            (true, false) => "runtime-stress",
            (false, true) => "runtime-startup-stats",
            (true, true) => "runtime-stress-startup-stats",
        })
        .join("index.cbor")
}
