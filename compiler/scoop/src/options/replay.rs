use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use scoop_protocol::HostPathCarrier;
use serde::{Deserialize, Serialize};

use super::{BuildOptions, LinkMode, MessageFormat};
use crate::{BuildFailure, BuildFailurePhase, BuildResult};

/// Successful CLI selection, independent of artifact and compile-cache identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildInvocation {
    pub cwd: HostPathCarrier,
    pub sysroot: HostPathCarrier,
    pub package_manager: HostPathCarrier,
    pub enable_gc_stress: bool,
    pub enable_startup_stats: bool,
    pub effective_debug_info: String,
    pub options: BuildOptions,
}

pub(super) fn absolute(cwd: &Path, path: PathBuf) -> BuildResult<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(config("path must not be empty"));
    }
    Ok(cwd.join(path))
}

fn config(error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_CLI_CONFIG", BuildFailurePhase::Request, error)
}

impl BuildOptions {
    /// Resolve explicit input paths without reading inputs or probing tools.
    pub fn normalized(mut self, cwd: &Path) -> BuildResult<Self> {
        for path in [
            &mut self.root_input,
            &mut self.target_dir,
            &mut self.scoopc,
            &mut self.cache_dir,
            &mut self.system_library_root,
            &mut self.runtime_root,
            &mut self.runtime_objects,
            &mut self.profile_use,
            &mut self.packages.lockfile,
            &mut self.packages.package_cache,
            &mut self.native.native_sysroot,
            &mut self.native.unwind_prefix,
        ] {
            *path = path.take().map(|value| absolute(cwd, value)).transpose()?;
        }
        for paths in [&mut self.cone_path, &mut self.library_path] {
            *paths = std::mem::take(paths)
                .into_iter()
                .map(|path| absolute(cwd, path))
                .collect::<BuildResult<_>>()?;
        }
        // Bare driver names retain ordinary PATH lookup; relative file names do not.
        for path in [
            &mut self.native.cc,
            &mut self.native.cxx,
            &mut self.native.linker,
        ] {
            if let Some(value) = path.as_mut()
                && value.components().count() > 1
            {
                *value = absolute(cwd, value.clone())?;
            }
        }
        self.generators = scoop_toolchain::GeneratorPrograms::from_cli(&self.generators, cwd)
            .and_then(|programs| programs.registrations())
            .map_err(config)?
            .into_iter()
            .map(|registration| {
                let mut argument = OsString::from(registration.name());
                argument.push("=");
                argument.push(registration.program().to_path_buf().map_err(config)?);
                Ok(argument)
            })
            .collect::<BuildResult<_>>()?;
        Ok(self)
    }

    /// Use the same declaration and defaults when invoking the separate driver.
    pub fn arguments(&self) -> Vec<OsString> {
        let mut args = Arguments::default();
        args.optional("--target", self.target.as_deref());
        args.optional("--profile", self.profile.map(|p| p.as_str()));
        args.flag("--release", self.release);
        args.optional("--debug-info", self.debug_info.map(|v| v.as_str()));
        args.value("--export-ir", self.export_ir.as_str());
        args.optional("--profile-use", self.profile_use.as_deref());
        args.flag("--profile-use-required", self.profile_use_required);
        args.optional("--target-dir", self.target_dir.as_deref());
        args.optional("--scoopc", self.scoopc.as_deref());
        args.optional("--cache-dir", self.cache_dir.as_deref());
        args.optional("--jobs", self.jobs.map(|v| v.to_string()));
        for value in &self.generators {
            args.value("--generator", value);
        }
        for value in &self.cone_path {
            args.value("--cone-path", value);
        }
        for value in &self.library_path {
            args.value("--library-path", value);
        }
        args.optional("--system-library-root", self.system_library_root.as_deref());
        args.optional("--runtime-root", self.runtime_root.as_deref());
        args.optional("--runtime-objects", self.runtime_objects.as_deref());
        args.value(
            "--message-format",
            match self.message_format {
                MessageFormat::Human => "human",
                MessageFormat::Json => "json",
            },
        );
        args.optional("--lockfile", self.packages.lockfile.as_deref());
        args.optional("--package-cache", self.packages.package_cache.as_deref());
        args.flag("--locked", self.packages.locked);
        args.flag("--offline", self.packages.offline);
        args.flag("--frozen", self.packages.frozen);
        args.flag("--workspace", self.packages.members.workspace);
        for value in &self.packages.members.package {
            args.value("--package", value.to_string());
        }
        args.optional("--cc", self.native.cc.as_deref());
        args.optional("--cxx", self.native.cxx.as_deref());
        args.optional("--linker", self.native.linker.as_deref());
        args.optional("--native-sysroot", self.native.native_sysroot.as_deref());
        args.optional("--unwind-prefix", self.native.unwind_prefix.as_deref());
        args.optional(
            "--link-mode",
            self.native.link_mode.map(|v| match v {
                LinkMode::Static => "static",
                LinkMode::Dynamic => "dynamic",
            }),
        );
        args.optional("--install-name", self.native.install_name.as_deref());
        args.optional("--soname", self.native.soname.as_deref());
        args.flag("--keep-symbols", self.native.keep_symbols);
        args.flag("--emit-debug-file", self.native.emit_debug_file);
        if let Some(value) = self.native.enable_stack_trace {
            args.0.push(format!("--enable-stack-trace={value}").into());
        }
        if let Some(root) = &self.root_input {
            args.0.push(root.as_os_str().to_owned());
        }
        args.0
    }
}

#[derive(Default)]
struct Arguments(Vec<OsString>);
impl Arguments {
    fn value(&mut self, flag: &str, value: impl AsRef<OsStr>) {
        self.0.push(flag.into());
        self.0.push(value.as_ref().to_owned());
    }
    fn optional(&mut self, flag: &str, value: Option<impl AsRef<OsStr>>) {
        if let Some(value) = value {
            self.value(flag, value);
        }
    }
    fn flag(&mut self, flag: &str, enabled: bool) {
        if enabled {
            self.0.push(flag.into());
        }
    }
}
