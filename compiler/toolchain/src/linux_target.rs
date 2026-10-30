//! The implemented Linux machine and libc facts, selected at the tool boundary.

use scoop_lir::{TargetArchitecture, TargetProfileId};
use scoop_target_data::NativeEnvironment;

use crate::ToolchainError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LinuxMachine {
    X86_64,
}

impl LinuxMachine {
    pub(crate) fn object_architecture(self) -> object::Architecture {
        match self {
            Self::X86_64 => object::Architecture::X86_64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LinuxLibc {
    Gnu,
    Musl,
}

impl LinuxLibc {
    pub(crate) fn default_c_driver(self) -> &'static str {
        match self {
            Self::Gnu => "gcc",
            Self::Musl => "musl-gcc",
        }
    }

    pub(crate) fn header_probe(self) -> &'static str {
        match self {
            Self::Gnu => "#ifndef __GLIBC__\n#error glibc target requires glibc headers\n#endif\n",
            Self::Musl => "#ifdef __GLIBC__\n#error musl target cannot use glibc headers\n#endif\n",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LinuxTarget {
    pub(crate) machine: LinuxMachine,
    pub(crate) libc: LinuxLibc,
}

impl LinuxTarget {
    pub(crate) fn select(target: TargetProfileId) -> Result<Self, ToolchainError> {
        let definition = scoop_target_data::definition(target)
            .ok_or_else(|| ToolchainError("unknown Linux target".into()))?;
        let libc = match definition.native_environment {
            NativeEnvironment::LinuxGnu => LinuxLibc::Gnu,
            NativeEnvironment::LinuxMusl => LinuxLibc::Musl,
            _ => {
                return Err(ToolchainError(
                    "Linux toolchain requires a Linux target".into(),
                ));
            }
        };
        let machine = match definition.selection.target().architecture() {
            TargetArchitecture::X86_64 => LinuxMachine::X86_64,
            architecture => {
                return Err(ToolchainError(format!(
                    "no Linux machine toolchain for {architecture:?}"
                )));
            }
        };
        Ok(Self { machine, libc })
    }
}
