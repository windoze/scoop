//! Native executable toolchains; artifact consumers do not load LLVM.
use std::path::{Path, PathBuf};
use std::process::Command;

use scoop_lir::{LirTargetProfile, TargetProfileId, ValidatedCBridgeToolchainInvocation};
use scoop_wire::{Digest256, Encoder, WireEncode, domain_separated_cbor_hash};

use crate::{CToolchainOptions, ResolvedTargetProfile, SystemProvider, ToolchainError};

mod darwin;
mod linux;
pub use darwin::DarwinFinalLinkProfile;
pub use linux::LinuxFinalLinkProfile;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkMode {
    Static,
    Dynamic,
}

#[derive(Clone, Debug, Default)]
pub struct FinalLinkOptions {
    pub sysroot: Option<PathBuf>,
    pub unwind_prefix: Option<PathBuf>,
    pub mode: Option<LinkMode>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidatedFinalLinkProfile {
    Darwin(DarwinFinalLinkProfile),
    Linux(LinuxFinalLinkProfile),
}

impl ValidatedFinalLinkProfile {
    pub fn resolve(triple: &str) -> Result<Self, ToolchainError> {
        Self::resolve_with(
            triple,
            &CToolchainOptions::default(),
            &FinalLinkOptions::default(),
        )
    }

    pub fn resolve_with(
        triple: &str,
        c_toolchain: &CToolchainOptions,
        options: &FinalLinkOptions,
    ) -> Result<Self, ToolchainError> {
        let target = ResolvedTargetProfile::resolve_with(triple, c_toolchain)?;
        Self::from_startup(target.c_bridge_toolchain().clone(), options, false)
    }

    pub(crate) fn from_startup(
        startup: ValidatedCBridgeToolchainInvocation,
        options: &FinalLinkOptions,
        cxx: bool,
    ) -> Result<Self, ToolchainError> {
        let cxx = cxx
            .then(|| crate::ValidatedCxxToolchain::resolve(&startup))
            .transpose()?;
        match startup.profile().contract().target().id() {
            TargetProfileId::DarwinAarch64 => {
                if options.mode == Some(LinkMode::Static) || options.unwind_prefix.is_some() {
                    return Err(error(
                        "Darwin uses dynamic libSystem unwinding; static mode and an unwind prefix are not applicable",
                    ));
                }
                DarwinFinalLinkProfile::from_startup(startup, cxx).map(Self::Darwin)
            }
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => {
                LinuxFinalLinkProfile::resolve(startup, options, cxx).map(Self::Linux)
            }
        }
    }

    pub fn id(&self) -> TargetProfileId {
        self.startup_toolchain().profile().contract().target().id()
    }

    pub fn cxx(&self) -> bool {
        match self {
            Self::Darwin(profile) => profile.cxx(),
            Self::Linux(profile) => profile.cxx(),
        }
    }

    pub fn target(&self) -> LirTargetProfile {
        LirTargetProfile::from_id(self.id())
    }

    pub fn canonical_triple(&self) -> &'static str {
        self.id().canonical_triple()
    }

    pub fn startup_toolchain(&self) -> &ValidatedCBridgeToolchainInvocation {
        match self {
            Self::Darwin(profile) => profile.startup_toolchain(),
            Self::Linux(profile) => profile.startup_toolchain(),
        }
    }

    pub fn compiler_digest(&self) -> Digest256 {
        match self {
            Self::Darwin(profile) => profile.compiler_digest(),
            Self::Linux(profile) => profile.compiler_digest(),
        }
    }

    pub fn linker_driver(&self) -> &Path {
        match self {
            Self::Darwin(profile) => profile.linker_driver(),
            Self::Linux(profile) => profile.linker_driver(),
        }
    }

    pub fn linker_args(&self) -> &'static [&'static str] {
        match self {
            Self::Darwin(profile) => profile.linker_args(),
            Self::Linux(profile) => profile.linker_args(),
        }
    }

    pub fn system_provider(&self) -> Result<&SystemProvider, ToolchainError> {
        match self {
            Self::Darwin(profile) => Ok(profile.system_provider()),
            Self::Linux(_) => Err(error(
                "ELF uses native CRT/libc inputs, not Darwin system stubs",
            )),
        }
    }

    pub fn linker_system_requirements(&self) -> &'static [&'static str] {
        match self {
            Self::Darwin(profile) => profile.linker_system_requirements(),
            Self::Linux(_) => &[],
        }
    }

    pub fn command(
        &self,
        scratch: &Path,
        output: &Path,
        map: &Path,
    ) -> Result<Command, ToolchainError> {
        match self {
            Self::Darwin(profile) => profile.command(scratch, output, map),
            Self::Linux(profile) => profile.command(scratch, output, map),
        }
    }

    pub fn fingerprint(&self) -> Result<Digest256, ToolchainError> {
        domain_separated_cbor_hash("scoop-final-link-profile-v2", self).map_err(error)
    }
}

impl WireEncode for ValidatedFinalLinkProfile {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Darwin(profile) => profile.encode(encoder),
            Self::Linux(profile) => profile.encode(encoder),
        }
    }
}

fn error(value: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("final-link toolchain: {value}"))
}
