//! Implemented ELF machine rules shared by object readers and the final linker.

use object::{Architecture, elf};
use scoop_identity::TargetProfileId;

use super::{ElfObjectError, error, x86_64};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElfArchitecture {
    X86_64,
}

impl ElfArchitecture {
    pub fn for_target(target: TargetProfileId) -> Result<Self, ElfObjectError> {
        match target {
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => Ok(Self::X86_64),
            TargetProfileId::DarwinAarch64 => Err(error("selected target does not support ELF")),
        }
    }

    pub fn object_architecture(self) -> Architecture {
        match self {
            Self::X86_64 => Architecture::X86_64,
        }
    }

    pub(super) fn check_flags(self, flags: u32) -> Result<(), ElfObjectError> {
        match self {
            Self::X86_64 if flags == 0 => Ok(()),
            Self::X86_64 => Err(error("nonzero flags in amd64 ELF header")),
        }
    }

    pub(super) fn relocation_width(self, kind: u32) -> Result<u8, ElfObjectError> {
        match self {
            Self::X86_64 => x86_64::relocation_width(kind),
        }
    }

    pub fn relative_pointer_relocation(self) -> u32 {
        match self {
            Self::X86_64 => elf::R_X86_64_RELATIVE,
        }
    }
}
