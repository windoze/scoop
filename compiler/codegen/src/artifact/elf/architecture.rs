//! ELF output rules for the backend machines implemented by codegen.

use object::{Architecture, elf};

use crate::{CodegenError, target::CodeArchitecture};

#[derive(Clone, Copy)]
pub(in crate::artifact) enum ElfArchitecture {
    X86_64,
}

impl ElfArchitecture {
    pub(in crate::artifact) fn select(
        architecture: CodeArchitecture,
    ) -> Result<Self, CodegenError> {
        match architecture {
            CodeArchitecture::X86_64 => Ok(Self::X86_64),
            CodeArchitecture::Aarch64 => Err(CodegenError(
                "codegen has no implemented ELF artifact rules for AArch64".into(),
            )),
        }
    }

    pub(in crate::artifact) fn object_architecture(self) -> Architecture {
        match self {
            Self::X86_64 => Architecture::X86_64,
        }
    }

    pub(in crate::artifact) fn absolute_pointer_relocation(self) -> u32 {
        match self {
            Self::X86_64 => elf::R_X86_64_64,
        }
    }

    pub(in crate::artifact) fn eh_pointer_relocation(self) -> u32 {
        match self {
            Self::X86_64 => elf::R_X86_64_PC32,
        }
    }

    pub(in crate::artifact) fn return_address_register(self) -> u8 {
        match self {
            Self::X86_64 => 16,
        }
    }
}
