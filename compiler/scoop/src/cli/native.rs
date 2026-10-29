use std::path::Path;

use scoop::{BuildFailure, BuildFailurePhase, BuildResult};
use scoop_protocol::TargetSelectionRequestV1;
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
                cxx_compiler: self.cxx,
                native_sysroot: self
                    .native_sysroot
                    .map(|path| absolute(cwd, path))
                    .transpose()?,
            },
            FinalLinkOptions {
                linker: self.linker,
                sysroot: Some(sysroot.to_owned()),
                unwind_prefix: self
                    .unwind_prefix
                    .map(|path| absolute(cwd, path))
                    .transpose()?,
                mode: self.link_mode.map(Into::into),
                keep_symbols: self.keep_symbols,
                emit_debug_file: self.emit_debug_file,
            },
        ))
    }
}

pub(super) fn target_request(
    triple: String,
    tools: &CToolchainOptions,
) -> BuildResult<TargetSelectionRequestV1> {
    tools
        .request(triple)
        .map_err(|error| BuildFailure::tool("SCOOP_CLI_CONFIG", BuildFailurePhase::Request, error))
}
