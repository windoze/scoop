use std::path::{Path, PathBuf};

use scoop::{BuildFailure, BuildFailurePhase, BuildResult};
use scoop_protocol::{HostPathCarrier, TargetSelectionRequestV1};
use scoop_toolchain::{CToolchainOptions, FinalLinkOptions};

use super::args::NativeOptions;
use super::commands::absolute;

impl NativeOptions {
    pub fn resolve(
        self,
        cwd: &Path,
        sysroot: &Path,
    ) -> BuildResult<(CToolchainOptions, FinalLinkOptions)> {
        Ok((
            CToolchainOptions {
                compiler: self.cc,
                native_sysroot: self
                    .native_sysroot
                    .map(|path| absolute(cwd, path))
                    .transpose()?,
            },
            FinalLinkOptions {
                sysroot: Some(sysroot.to_owned()),
                unwind_prefix: self
                    .unwind_prefix
                    .map(|path| absolute(cwd, path))
                    .transpose()?,
                mode: self.link_mode.map(Into::into),
            },
        ))
    }
}

pub(super) fn target_request(
    triple: String,
    tools: &CToolchainOptions,
) -> BuildResult<TargetSelectionRequestV1> {
    let path = |value: &Option<PathBuf>| {
        value
            .as_deref()
            .map(HostPathCarrier::from_path)
            .transpose()
            .map_err(|error| {
                BuildFailure::tool("SCOOP_CLI_CONFIG", BuildFailurePhase::Request, error)
            })
    };
    Ok(TargetSelectionRequestV1::new(triple)
        .map_err(|error| BuildFailure::tool("SCOOP_CLI_CONFIG", BuildFailurePhase::Request, error))?
        .with_c_toolchain(path(&tools.compiler)?, path(&tools.native_sysroot)?))
}

pub(super) fn default_cache() -> Option<PathBuf> {
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
