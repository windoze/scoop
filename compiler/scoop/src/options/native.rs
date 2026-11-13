use super::NativeOptions;
use super::replay::absolute;
use crate::BuildResult;
use scoop_toolchain::{CToolchainOptions, FinalLinkOptions};
use std::path::Path;

impl NativeOptions {
    pub fn resolve(
        self,
        cwd: &Path,
        sysroot: &Path,
    ) -> BuildResult<(CToolchainOptions, FinalLinkOptions)> {
        Ok((
            CToolchainOptions {
                sysroot: Some(sysroot.to_owned()),
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
                install_name: self.install_name,
                soname: self.soname,
                enable_stack_trace: self.enable_stack_trace,
                ..Default::default()
            },
        ))
    }
}
