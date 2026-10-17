use super::*;
use crate::linux_target::{LinuxLibc, LinuxTarget};
use scoop_process::CommandExt;
use scoop_wire::sha256;

mod probe;
mod runtime;
use runtime::RuntimeLibraries;
mod wire;

const OPTIONS: &[&str] = &[
    "-pthread",
    "-Wl,--eh-frame-hdr",
    "-Wl,--enable-new-dtags",
    "-Wl,--build-id=none",
    "-Wl,-z,relro,-z,now,-z,noexecstack,-z,text",
    "-Wl,--no-undefined",
];

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
struct SystemInput {
    #[serde(with = "crate::build_record::values::path")]
    path: PathBuf,
    #[serde(with = "crate::build_record::values::digest")]
    digest: Digest256,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxFinalLinkProfile {
    startup: ValidatedCBridgeToolchainInvocation,
    target: LinuxTarget,
    pub(super) data: Discovery,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Discovery {
    #[serde(with = "crate::build_record::values::paths")]
    pub(super) native_roots: Vec<PathBuf>,
    mode: LinkMode,
    #[serde(with = "crate::build_record::values::path")]
    linker: PathBuf,
    linker_version: String,
    #[serde(with = "crate::build_record::values::digest")]
    compiler_digest: Digest256,
    runtime: RuntimeLibraries,
    inputs: Vec<SystemInput>,
    #[serde(with = "crate::build_record::values::paths")]
    observed_paths: Vec<PathBuf>,
}

impl LinuxFinalLinkProfile {
    pub(super) fn into_discovery(self) -> Discovery {
        self.data
    }

    pub(super) fn from_discovery(
        startup: ValidatedCBridgeToolchainInvocation,
        data: Discovery,
    ) -> Result<Self, ToolchainError> {
        let target = LinuxTarget::select(startup.target().id())?;
        Ok(Self {
            startup,
            target,
            data,
        })
    }

    pub(super) fn resolve(
        startup: ValidatedCBridgeToolchainInvocation,
        options: &FinalLinkOptions,
        cxx: Option<crate::ValidatedCxxToolchain>,
        native_roots: Vec<PathBuf>,
    ) -> Result<Self, ToolchainError> {
        let target = startup.profile().contract().target().id();
        let selected = LinuxTarget::select(target)?;
        let mode = options.mode.unwrap_or(match selected.libc {
            LinuxLibc::Musl => LinkMode::Static,
            LinuxLibc::Gnu => LinkMode::Dynamic,
        });
        if selected.libc == LinuxLibc::Gnu && mode == LinkMode::Static {
            return Err(error(
                "M28 supports glibc dynamic PIE; glibc static linking is outside its target matrix",
            ));
        }
        let runtime = RuntimeLibraries::resolve(startup.target(), options, cxx)?;
        let mut observed_paths = Vec::new();
        let linker = probe::driver_program(&startup, "ld", &mut observed_paths)?;
        let version = probe::run(Command::new(&linker).arg("--version"))?;
        let linker_version = String::from_utf8(version.stdout).map_err(error)?;
        if !linker_version.starts_with("GNU ld ") {
            return Err(error("this Linux driver profile requires GNU ld"));
        }
        let compiler_digest = crate::build_record::fingerprint(&crate::build_record::Input::read(
            runtime.driver(&startup),
        )?)?;
        let mut profile = Self {
            startup,
            target: selected,
            data: Discovery {
                native_roots,
                mode,
                linker,
                linker_version,
                compiler_digest,
                runtime,
                inputs: Vec::new(),
                observed_paths: Vec::new(),
            },
        };
        observed_paths.extend(probe::search_paths(&profile.startup, &profile.data.linker)?);
        profile.data.inputs = probe::check(&profile, &mut observed_paths)?;
        profile.data.observed_paths = observed_paths;
        Ok(profile)
    }

    pub fn mode(&self) -> LinkMode {
        self.data.mode
    }
    pub fn startup_toolchain(&self) -> &ValidatedCBridgeToolchainInvocation {
        &self.startup
    }
    pub fn compiler_digest(&self) -> Digest256 {
        self.data.compiler_digest
    }
    pub fn linker_driver(&self) -> &Path {
        &self.data.linker
    }
    pub fn linker_args(&self) -> &'static [&'static str] {
        OPTIONS
    }
    pub fn unwind_prefix(&self) -> Option<&Path> {
        self.data.runtime.unwind_prefix()
    }
    pub fn cxx(&self) -> bool {
        self.data.runtime.cxx()
    }
    pub fn input_paths(&self) -> impl Iterator<Item = &Path> {
        self.data.inputs.iter().map(|input| input.path.as_path())
    }

    pub(super) fn update_input_paths(&self) -> Result<Vec<PathBuf>, ToolchainError> {
        let mut paths = self.input_paths().map(PathBuf::from).collect::<Vec<_>>();
        paths.extend(self.data.observed_paths.iter().cloned());
        paths.push(self.data.runtime.driver(&self.startup).to_owned());
        if let RuntimeLibraries::Cxx(cxx) = &self.data.runtime {
            paths.extend(cxx.input_paths()?);
        }
        Ok(paths)
    }

    pub fn command(&self, scratch: &Path, output: &Path) -> Result<Command, ToolchainError> {
        std::fs::create_dir_all(scratch).map_err(error)?;
        let script = scratch.join("scoop-metadata.ld");
        std::fs::write(&script, self.metadata_script()).map_err(error)?;
        let mut command = self.data.runtime.command(&self.startup);
        if !self.cxx() {
            command.arg("-nodefaultlibs");
        }
        command
            .env("TMPDIR", scratch)
            .args(OPTIONS)
            .args(self.mode_flags())
            .arg("-o")
            .arg(output)
            .args(["-Xlinker", "-T", "-Xlinker"])
            .arg(script);
        Ok(command)
    }

    /// Append after the ordinary objects, so archive extraction sees their uses.
    pub fn append_system_libraries(&self, command: &mut Command) {
        if let Some(prefix) = self.unwind_prefix() {
            command
                .arg("-Wl,--start-group")
                .arg(prefix.join("lib/libunwind.a"))
                .args(["-lc", "-lm", "-lpthread", "-lgcc", "-Wl,--end-group"]);
        }
    }

    fn metadata_script(&self) -> &'static str {
        match self.data.mode {
            LinkMode::Static => include_str!("../elf/static.ld"),
            LinkMode::Dynamic => include_str!("../elf/dynamic.ld"),
        }
    }

    fn mode_flags(&self) -> &'static [&'static str] {
        match self.data.mode {
            LinkMode::Static => &["-static", "-no-pie"],
            LinkMode::Dynamic => &["-pie"],
        }
    }
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;
