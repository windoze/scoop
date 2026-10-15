use std::path::{Path, PathBuf};

pub fn configured_cache_root(explicit: Option<PathBuf>) -> Result<PathBuf, crate::ToolchainError> {
    explicit
        .or_else(|| std::env::var_os("SCOOP_CACHE_DIR").map(PathBuf::from))
        .or_else(default_cache)
        .ok_or_else(|| {
            crate::ToolchainError("no usable HOME; provide --cache-dir or SCOOP_CACHE_DIR".into())
        })
}

fn default_cache() -> Option<PathBuf> {
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
