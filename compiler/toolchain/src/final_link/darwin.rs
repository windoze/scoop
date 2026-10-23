//! Explicit Darwin executable toolchain; independent of the Scoop backend.
use scoop_process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ValidatedCBridgeToolchainInvocation;
use scoop_lir::{LirTargetProfile, TargetProfileId};
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash};

use crate::{SystemProvider, ToolchainError, c_bridge};

#[path = "probe.rs"]
mod probe;

const OPTIONS: &[&str] = &[
    "-arch",
    "arm64",
    "-execute",
    "-pie",
    "-e",
    "_main",
    "-no_deduplicate",
    "-no_fixup_chains",
    "-no_implicit_dylibs",
    "-adhoc_codesign",
    "-unexported_symbol",
    "_scoop$1$bs$*",
    "-unexported_symbol",
    "_scoop$1$be$*",
    "-rename_section",
    "__LLVM_STACKMAPS",
    "__llvm_stackmaps",
    "__DATA_CONST",
    "__llvm_stackmaps",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DarwinFinalLinkProfile {
    startup: ValidatedCBridgeToolchainInvocation,
    pub(super) data: Discovery,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct Discovery {
    #[serde(with = "crate::build_record::values::paths")]
    pub(super) native_roots: Vec<PathBuf>,
    #[serde(with = "crate::build_record::values::path")]
    linker: PathBuf,
    #[serde(with = "crate::build_record::values::path")]
    linker_locator: PathBuf,
    linker_version: String,
    #[serde(with = "crate::build_record::values::digest")]
    linker_digest: Digest256,
    #[serde(with = "crate::build_record::values::digest")]
    compiler_digest: Digest256,
    system: SystemProvider,
    cxx: Option<crate::ValidatedCxxToolchain>,
    #[serde(with = "crate::build_record::values::paths")]
    headers: Vec<PathBuf>,
}

impl DarwinFinalLinkProfile {
    pub(super) fn into_discovery(self) -> Discovery {
        self.data
    }

    pub(super) fn from_discovery(
        startup: ValidatedCBridgeToolchainInvocation,
        data: Discovery,
    ) -> Self {
        Self { startup, data }
    }

    pub(crate) fn from_startup(
        startup: ValidatedCBridgeToolchainInvocation,
        cxx: Option<crate::ValidatedCxxToolchain>,
        native_roots: Vec<PathBuf>,
    ) -> Result<Self, ToolchainError> {
        let path =
            c_bridge::command_text_from_path(Path::new("/usr/bin/xcrun"), &["--find", "ld"])?;
        let linker = std::fs::canonicalize(path.trim()).map_err(error)?;
        let linker_locator = PathBuf::from(path.trim());
        let linker_version = linker_version(&linker)?;
        let linker_digest =
            crate::build_record::fingerprint(&crate::build_record::Input::read(&linker)?)?;
        let compiler_digest = crate::build_record::fingerprint(&crate::build_record::Input::read(
            cxx.as_ref()
                .map_or(startup.compiler_driver(), |cxx| cxx.driver()),
        )?)?;
        let system = SystemProvider::read(
            startup.sdk_root().map_err(error)?,
            startup
                .profile()
                .contract()
                .deployment()
                .map_err(error)?
                .minimum_os(),
        )?;
        let mut profile = Self {
            startup,
            data: Discovery {
                native_roots,
                linker,
                linker_locator,
                linker_version,
                linker_digest,
                compiler_digest,
                system,
                cxx,
                headers: Vec::new(),
            },
        };
        profile.data.headers = probe::check(&profile)?;
        Ok(profile)
    }

    pub const fn id(&self) -> TargetProfileId {
        scoop_target_data::DARWIN_AARCH64_ID
    }
    pub const fn target(&self) -> LirTargetProfile {
        scoop_target_data::DARWIN_AARCH64
    }
    pub const fn canonical_triple(&self) -> &'static str {
        "aarch64-apple-darwin"
    }
    pub fn linker_driver(&self) -> &Path {
        &self.data.linker
    }
    pub fn linker_args(&self) -> &'static [&'static str] {
        OPTIONS
    }
    pub fn startup_toolchain(&self) -> &ValidatedCBridgeToolchainInvocation {
        &self.startup
    }
    pub fn compiler_digest(&self) -> Digest256 {
        self.data.compiler_digest
    }
    pub fn system_provider(&self) -> &SystemProvider {
        &self.data.system
    }

    pub(super) fn input_paths(&self) -> Result<Vec<PathBuf>, ToolchainError> {
        let sdk = self.startup.sdk_root().map_err(error)?;
        let mut paths = vec![
            self.data.linker_locator.clone(),
            self.startup.compiler_driver().to_owned(),
        ];
        paths.extend(
            self.data
                .system
                .files()
                .iter()
                .map(|file| sdk.join(file.relative_path())),
        );
        paths.extend(self.data.headers.iter().cloned());
        if let Some(cxx) = &self.data.cxx {
            paths.extend(cxx.input_paths()?);
        }
        Ok(paths)
    }

    pub fn cxx(&self) -> bool {
        self.data.cxx.is_some()
    }

    pub fn linker_argument(&self, command: &mut Command, argument: impl AsRef<std::ffi::OsStr>) {
        if self.cxx() {
            command.arg("-Xlinker");
        }
        command.arg(argument);
    }

    pub fn linker_system_requirements(&self) -> &'static [&'static str] {
        &["dyld_stub_binder"]
    }

    pub fn fingerprint(&self) -> Result<Digest256, ToolchainError> {
        domain_separated_cbor_hash("scoop-final-link-profile-v2", self).map_err(error)
    }

    /// Callers append the ordered objects, aliases and selected system stubs.
    pub fn command(&self, sdk_root: &Path, output: &Path) -> Result<Command, ToolchainError> {
        let deployment = self
            .startup
            .profile()
            .contract()
            .deployment()
            .map_err(error)?;
        let mut command = match &self.data.cxx {
            Some(cxx) => {
                let mut command = cxx.command(&self.startup);
                // The link plan supplies the selected SDK and C++ stubs explicitly.
                command.arg("-nostdlib");
                command
            }
            None => {
                let mut command = Command::new(&self.data.linker);
                command
                    .env_clear()
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .env("TZ", "UTC");
                command
            }
        };
        for argument in OPTIONS {
            self.linker_argument(&mut command, argument);
        }
        for argument in [
            "-platform_version".into(),
            "macos".into(),
            deployment.minimum_os().to_string().into(),
            deployment.sdk().to_string().into(),
            "-syslibroot".into(),
            sdk_root.as_os_str().to_owned(),
            "-o".into(),
            output.as_os_str().to_owned(),
        ] {
            self.linker_argument(&mut command, argument);
        }
        Ok(command)
    }
}

impl WireEncode for DarwinFinalLinkProfile {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(10)?;
        e.field(1)?;
        self.target().wire_id().encode(e)?;
        e.field(2)?;
        self.startup.profile().contract().encode(e)?;
        e.field(3)?;
        self.data.compiler_digest.encode(e)?;
        e.field(4)?;
        self.data.linker_digest.encode(e)?;
        e.field(5)?;
        e.text(&self.data.linker_version)?;
        e.field(6)?;
        e.array(OPTIONS.len() as u64)?;
        for option in OPTIONS {
            e.text(option)?;
        }
        e.field(7)?;
        e.array(self.data.system.files().len() as u64)?;
        for file in self.data.system.files() {
            e.map(2)?;
            e.field(1)?;
            e.text(&file.relative_path().to_string_lossy())?;
            e.field(2)?;
            file.digest().encode(e)?;
        }
        e.field(8)?;
        e.array(self.linker_system_requirements().len() as u64)?;
        for symbol in self.linker_system_requirements() {
            e.text(symbol)?;
        }
        e.field(9)?;
        e.unsigned(1)?;
        e.field(10)?;
        e.array(u64::from(self.data.cxx.is_some()))?;
        if let Some(cxx) = &self.data.cxx {
            cxx.fingerprint().encode(e)?;
        }
        Ok(())
    }
}

fn error(value: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("final-link toolchain: {value}"))
}

fn linker_version(linker: &Path) -> Result<String, ToolchainError> {
    let output = Command::new(linker)
        .env_clear()
        .env("LC_ALL", "C")
        .arg("-v")
        .scoop_output()
        .map_err(error)?;
    if !output.status.success() {
        return Err(error(format!(
            "ld version query failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let bytes = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    std::str::from_utf8(bytes)
        .map_err(error)?
        .lines()
        .find(|line| line.starts_with("@(#)PROGRAM:ld "))
        .map(str::to_owned)
        .ok_or_else(|| error("ld did not report its version"))
}
