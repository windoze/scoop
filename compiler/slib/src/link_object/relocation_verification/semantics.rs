//! Architecture-specific relocation uses shared by bridge and runtime readers.

use object::elf::*;
use scoop_lir::{LirTargetProfile, NativeObjectFormat, TargetArchitecture};

use super::VerifiedObjectRelocationFormV1 as Form;

impl Form {
    pub(in crate::link_object) fn is_tls_reference(self, target: LirTargetProfile) -> bool {
        match (target.native_object_format(), target.architecture()) {
            (NativeObjectFormat::Coff64, TargetArchitecture::X86_64) => matches!(
                self,
                Self::CoffAmd64 {
                    kind: object::pe::IMAGE_REL_AMD64_SECREL,
                    width: 4
                }
            ),
            (NativeObjectFormat::MachO64, TargetArchitecture::Aarch64) => {
                matches!(self, Self::TlvpLoadPage21 | Self::TlvpLoadPageOffset12)
            }
            (NativeObjectFormat::MachO64, TargetArchitecture::X86_64) => matches!(
                self,
                Self::X86_64PcRelative32 {
                    kind: super::MachOX86_64PcRelativeKindV1::Tlv
                }
            ),
            (NativeObjectFormat::Elf64, TargetArchitecture::X86_64) => {
                matches!(
                    self,
                    Self::ElfX86_64Rela {
                        kind: R_X86_64_TLSGD
                            | R_X86_64_TLSLD
                            | R_X86_64_DTPOFF32
                            | R_X86_64_GOTTPOFF
                            | R_X86_64_TPOFF32
                            | R_X86_64_GOTPC32_TLSDESC,
                        width: 4,
                        ..
                    } | Self::ElfX86_64Rela {
                        kind: R_X86_64_DTPOFF64 | R_X86_64_TPOFF64,
                        width: 8,
                        ..
                    }
                )
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amd64_tls_references_are_distinct_from_ordinary_addresses() {
        for target in [
            scoop_target_data::LINUX_X86_64_GNU,
            scoop_target_data::LINUX_X86_64_MUSL,
        ] {
            let data = Form::ElfX86_64Rela {
                kind: R_X86_64_REX_GOTPCRELX,
                width: 4,
                addend: -4,
            };
            let tls = Form::ElfX86_64Rela {
                kind: R_X86_64_TLSGD,
                width: 4,
                addend: -4,
            };
            assert!(!data.is_tls_reference(target));
            assert!(tls.is_tls_reference(target));
            assert!(!Form::TlvpLoadPage21.is_tls_reference(target));
            assert!(!tls.is_tls_reference(scoop_target_data::DARWIN_AARCH64));
        }
    }
}
