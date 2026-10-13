//! The implemented Linux machine and libc facts, selected at the tool boundary.

use scoop_lir::TargetProfileId;

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
        let (machine, libc) = match target {
            scoop_target_data::LINUX_X86_64_GNU_ID => (LinuxMachine::X86_64, LinuxLibc::Gnu),
            scoop_target_data::LINUX_X86_64_MUSL_ID => (LinuxMachine::X86_64, LinuxLibc::Musl),
            _ => {
                return Err(ToolchainError(
                    "Linux toolchain requires a Linux target".into(),
                ));
            }
        };
        Ok(Self { machine, libc })
    }

    pub(crate) fn interpreter_name(self) -> &'static str {
        match (self.machine, self.libc) {
            (LinuxMachine::X86_64, LinuxLibc::Gnu) => "ld-linux-x86-64.so.2",
            (LinuxMachine::X86_64, LinuxLibc::Musl) => "ld-musl-x86_64.so.1",
        }
    }
}
