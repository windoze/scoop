use super::*;
use scoop_process::CommandExt;
use scoop_wire::sha256;

mod image;
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct SystemInput {
    path: PathBuf,
    digest: Digest256,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinuxFinalLinkProfile {
    startup: ValidatedCBridgeToolchainInvocation,
    mode: LinkMode,
    linker: PathBuf,
    linker_version: String,
    compiler_digest: Digest256,
    runtime: RuntimeLibraries,
    inputs: Vec<SystemInput>,
}

impl LinuxFinalLinkProfile {
    pub(super) fn resolve(
        startup: ValidatedCBridgeToolchainInvocation,
        options: &FinalLinkOptions,
        cxx: Option<crate::ValidatedCxxToolchain>,
    ) -> Result<Self, ToolchainError> {
        let target = startup.profile().contract().target().id();
        let mode = options.mode.unwrap_or(match target {
            TargetProfileId::LinuxX86_64Musl => LinkMode::Static,
            TargetProfileId::LinuxX86_64Gnu => LinkMode::Dynamic,
            TargetProfileId::DarwinAarch64 => {
                return Err(error("Linux final link requires an ELF target"));
            }
        });
        if target == TargetProfileId::LinuxX86_64Gnu && mode == LinkMode::Static {
            return Err(error(
                "M28 supports glibc dynamic PIE; glibc static linking is outside its target matrix",
            ));
        }
        let runtime = RuntimeLibraries::resolve(target, options, cxx)?;
        let linker = probe::driver_program(&startup, "ld")?;
        let version = probe::run(Command::new(&linker).arg("--version"))?;
        let linker_version = String::from_utf8(version.stdout).map_err(error)?;
        if !linker_version.starts_with("GNU ld ") {
            return Err(error("this Linux driver profile requires GNU ld"));
        }
        let compiler_digest = sha256(&std::fs::read(runtime.driver(&startup)).map_err(error)?);
        let mut profile = Self {
            startup,
            mode,
            linker,
            linker_version,
            compiler_digest,
            runtime,
            inputs: Vec::new(),
        };
        profile.inputs = probe::check(&profile)?;
        Ok(profile)
    }

    pub fn mode(&self) -> LinkMode {
        self.mode
    }
    pub fn startup_toolchain(&self) -> &ValidatedCBridgeToolchainInvocation {
        &self.startup
    }
    pub fn compiler_digest(&self) -> Digest256 {
        self.compiler_digest
    }
    pub fn linker_driver(&self) -> &Path {
        &self.linker
    }
    pub fn linker_args(&self) -> &'static [&'static str] {
        OPTIONS
    }
    pub fn unwind_prefix(&self) -> Option<&Path> {
        self.runtime.unwind_prefix()
    }
    pub fn cxx(&self) -> bool {
        self.runtime.cxx()
    }
    pub fn input_paths(&self) -> impl Iterator<Item = &Path> {
        self.inputs.iter().map(|input| input.path.as_path())
    }

    pub fn command(
        &self,
        scratch: &Path,
        output: &Path,
        map: &Path,
    ) -> Result<Command, ToolchainError> {
        std::fs::create_dir_all(scratch).map_err(error)?;
        let script = scratch.join("scoop-metadata.ld");
        std::fs::write(&script, self.metadata_script()).map_err(error)?;
        let mut command = self.runtime.command(&self.startup);
        if !self.cxx() {
            command.arg("-nodefaultlibs");
        }
        command
            .env("TMPDIR", scratch)
            .args(OPTIONS)
            .args(self.mode_flags())
            .arg("-o")
            .arg(output)
            .args(["-Xlinker", "-Map", "-Xlinker"])
            .arg(map)
            .args(["-Xlinker", "-T", "-Xlinker"])
            .arg(script)
            .arg("-Wl,-t");
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

    pub fn check_image(&self, bytes: &[u8]) -> Result<(), ToolchainError> {
        image::check(self, bytes)
    }

    fn metadata_script(&self) -> &'static str {
        match self.mode {
            LinkMode::Static => include_str!("../elf/static.ld"),
            LinkMode::Dynamic => include_str!("../elf/dynamic.ld"),
        }
    }

    fn mode_flags(&self) -> &'static [&'static str] {
        match self.mode {
            LinkMode::Static => &["-static", "-no-pie"],
            LinkMode::Dynamic => &["-pie"],
        }
    }
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;
