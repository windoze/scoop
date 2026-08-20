use std::path::{Path, PathBuf};

/// Build-time development layout; explicit CLI paths allow relocating tools.
pub fn development_workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("toolchain crate lives under compiler in the workspace")
        .to_owned()
}

pub fn configured_sysroot_root(explicit: Option<PathBuf>) -> PathBuf {
    explicit
        .or_else(|| std::env::var_os("SCOOP_SYSROOT").map(PathBuf::from))
        .unwrap_or_else(|| development_workspace_root().join("sysroot"))
}

pub fn development_runtime_root() -> PathBuf {
    development_workspace_root().join("runtime")
}
