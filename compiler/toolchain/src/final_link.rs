//! Explicit Darwin executable toolchain; independent of the Scoop backend.
use std::path::{Path, PathBuf};
use std::process::Command;

use scoop_lir::{LirTargetProfile, TargetProfileId, ValidatedCBridgeToolchainInvocation};
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash, sha256};

use crate::{SystemProvider, ToolchainError, c_bridge};

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
    "-rename_section",
    "__LLVM_STACKMAPS",
    "__llvm_stackmaps",
    "__DATA_CONST",
    "__llvm_stackmaps",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedFinalLinkProfile {
    startup: ValidatedCBridgeToolchainInvocation,
    linker: PathBuf,
    linker_version: String,
    linker_digest: Digest256,
    compiler_digest: Digest256,
    system: SystemProvider,
}

impl ValidatedFinalLinkProfile {
    /// Resolves only the tools used by artifact consumption, without LLVM.
    pub fn resolve(triple: &str) -> Result<Self, ToolchainError> {
        crate::registry::validate_target(triple)?;
        Self::from_startup(c_bridge::resolve_system_c_bridge_toolchain()?)
    }

    pub(crate) fn from_startup(
        startup: ValidatedCBridgeToolchainInvocation,
    ) -> Result<Self, ToolchainError> {
        let path =
            c_bridge::command_text_from_path(Path::new("/usr/bin/xcrun"), &["--find", "ld"])?;
        let linker = std::fs::canonicalize(path.trim()).map_err(error)?;
        let linker_version = linker_version(&linker)?;
        let linker_digest = sha256(&std::fs::read(&linker).map_err(error)?);
        let compiler_digest = sha256(&std::fs::read(startup.compiler_driver()).map_err(error)?);
        let system = SystemProvider::read(
            startup.sdk_root(),
            startup.profile().contract().deployment().minimum_os(),
        )?;
        let profile = Self {
            startup,
            linker,
            linker_version,
            linker_digest,
            compiler_digest,
            system,
        };
        probe::check(&profile)?;
        Ok(profile)
    }

    pub const fn id(&self) -> TargetProfileId {
        TargetProfileId::DarwinAarch64
    }
    pub const fn target(&self) -> LirTargetProfile {
        LirTargetProfile::DARWIN_AARCH64
    }
    pub const fn canonical_triple(&self) -> &'static str {
        "aarch64-apple-darwin"
    }
    pub fn linker_driver(&self) -> &Path {
        &self.linker
    }
    pub fn linker_args(&self) -> &'static [&'static str] {
        OPTIONS
    }
    pub fn startup_toolchain(&self) -> &ValidatedCBridgeToolchainInvocation {
        &self.startup
    }
    pub fn compiler_digest(&self) -> Digest256 {
        self.compiler_digest
    }
    pub fn system_provider(&self) -> &SystemProvider {
        &self.system
    }

    pub fn linker_system_requirements(&self) -> &'static [&'static str] {
        &["dyld_stub_binder"]
    }

    pub fn fingerprint(&self) -> Result<Digest256, ToolchainError> {
        domain_separated_cbor_hash("scoop-final-link-profile-v2", self).map_err(error)
    }

    /// Callers append the ordered objects, aliases and snapshotted system stub.
    pub fn command(&self, sdk_snapshot: &Path, output: &Path, map: &Path) -> Command {
        let deployment = self.startup.profile().contract().deployment();
        let mut command = Command::new(&self.linker);
        command
            .env_clear()
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("TZ", "UTC")
            .args(OPTIONS)
            .arg("-platform_version")
            .arg("macos")
            .arg(deployment.minimum_os().to_string())
            .arg(deployment.sdk().to_string())
            .arg("-syslibroot")
            .arg(sdk_snapshot)
            .arg("-o")
            .arg(output)
            .arg("-map")
            .arg(map)
            .arg("-t");
        command
    }
}

impl WireEncode for ValidatedFinalLinkProfile {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(9)?;
        e.field(1)?;
        self.target().wire_id().encode(e)?;
        e.field(2)?;
        self.startup.profile().contract().encode(e)?;
        e.field(3)?;
        self.compiler_digest.encode(e)?;
        e.field(4)?;
        self.linker_digest.encode(e)?;
        e.field(5)?;
        e.text(&self.linker_version)?;
        e.field(6)?;
        e.array(OPTIONS.len() as u64)?;
        for option in OPTIONS {
            e.text(option)?;
        }
        e.field(7)?;
        e.array(self.system.files().len() as u64)?;
        for file in self.system.files() {
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
        e.unsigned(1)
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
        .output()
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
