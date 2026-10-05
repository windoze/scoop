//! Architecture-specific relocation uses shared by bridge and runtime readers.

use object::elf::*;
use scoop_lir::{LirTargetProfile, TargetProfileId};

use super::VerifiedObjectRelocationFormV1 as Form;

impl Form {
    pub(in crate::link_object) fn is_direct_call(self, target: LirTargetProfile) -> bool {
        match target.id() {
            TargetProfileId::DarwinAarch64 => self == Self::Branch26,
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => matches!(
                self,
                Self::ElfRela {
                    kind: R_X86_64_PC32 | R_X86_64_PLT32,
                    width: 4,
                    addend: -4
                }
            ),
        }
    }

    pub(in crate::link_object) fn is_data_address(self, target: LirTargetProfile) -> bool {
        match target.id() {
            TargetProfileId::DarwinAarch64 => matches!(
                self,
                Self::Unsigned64
                    | Self::Page21 { .. }
                    | Self::PageOffset12 { .. }
                    | Self::GotLoadPage21
                    | Self::GotLoadPageOffset12
                    | Self::PointerToGot32
            ),
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => matches!(
                self,
                Self::ElfRela {
                    kind: R_X86_64_64,
                    width: 8,
                    ..
                } | Self::ElfRela {
                    kind: R_X86_64_PC32
                        | R_X86_64_GOTPCREL
                        | R_X86_64_GOTPCRELX
                        | R_X86_64_REX_GOTPCRELX,
                    width: 4,
                    ..
                }
            ),
        }
    }

    pub(in crate::link_object) fn is_tls_reference(self, target: LirTargetProfile) -> bool {
        match target.id() {
            TargetProfileId::DarwinAarch64 => {
                matches!(self, Self::TlvpLoadPage21 | Self::TlvpLoadPageOffset12)
            }
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => matches!(
                self,
                Self::ElfRela {
                    kind: R_X86_64_TLSGD
                        | R_X86_64_TLSLD
                        | R_X86_64_DTPOFF32
                        | R_X86_64_GOTTPOFF
                        | R_X86_64_TPOFF32
                        | R_X86_64_GOTPC32_TLSDESC,
                    width: 4,
                    ..
                } | Self::ElfRela {
                    kind: R_X86_64_DTPOFF64 | R_X86_64_TPOFF64,
                    width: 8,
                    ..
                }
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amd64_calls_addresses_and_tls_do_not_substitute_for_each_other() {
        for target in [
            LirTargetProfile::LINUX_X86_64_GNU,
            LirTargetProfile::LINUX_X86_64_MUSL,
        ] {
            let call = Form::ElfRela {
                kind: R_X86_64_PLT32,
                width: 4,
                addend: -4,
            };
            let inside_function = Form::ElfRela {
                kind: R_X86_64_PLT32,
                width: 4,
                addend: 0,
            };
            let data = Form::ElfRela {
                kind: R_X86_64_REX_GOTPCRELX,
                width: 4,
                addend: -4,
            };
            let tls = Form::ElfRela {
                kind: R_X86_64_TLSGD,
                width: 4,
                addend: -4,
            };
            assert!(call.is_direct_call(target));
            assert!(!inside_function.is_direct_call(target));
            assert!(data.is_data_address(target));
            assert!(!data.is_tls_reference(target));
            assert!(tls.is_tls_reference(target));
            assert!(!tls.is_data_address(target));
            assert!(!tls.is_direct_call(target));
            assert!(!Form::Branch26.is_direct_call(target));
            assert!(!Form::TlvpLoadPage21.is_tls_reference(target));
            assert!(!call.is_direct_call(LirTargetProfile::DARWIN_AARCH64));
            assert!(!tls.is_tls_reference(LirTargetProfile::DARWIN_AARCH64));
        }
    }
}
