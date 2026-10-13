//! Architecture-specific relocation uses shared by bridge and runtime readers.

use object::elf::*;
use scoop_lir::LirTargetProfile;

use super::VerifiedObjectRelocationFormV1 as Form;

impl Form {
    pub(in crate::link_object) fn is_direct_call(self, target: LirTargetProfile) -> bool {
        match target.id() {
            scoop_target_data::DARWIN_AARCH64_ID => self == Self::Branch26,
            scoop_target_data::LINUX_X86_64_GNU_ID | scoop_target_data::LINUX_X86_64_MUSL_ID => {
                matches!(
                    self,
                    Self::ElfX86_64Rela {
                        kind: R_X86_64_PC32 | R_X86_64_PLT32,
                        width: 4,
                        addend: -4
                    }
                )
            }
            _ => false,
        }
    }

    pub(in crate::link_object) fn is_data_address(self, target: LirTargetProfile) -> bool {
        match target.id() {
            scoop_target_data::DARWIN_AARCH64_ID => matches!(
                self,
                Self::Unsigned64
                    | Self::Page21 { .. }
                    | Self::PageOffset12 { .. }
                    | Self::GotLoadPage21
                    | Self::GotLoadPageOffset12
                    | Self::PointerToGot32
            ),
            scoop_target_data::LINUX_X86_64_GNU_ID | scoop_target_data::LINUX_X86_64_MUSL_ID => {
                matches!(
                    self,
                    Self::ElfX86_64Rela {
                        kind: R_X86_64_64,
                        width: 8,
                        ..
                    } | Self::ElfX86_64Rela {
                        kind: R_X86_64_PC32
                            | R_X86_64_GOTPCREL
                            | R_X86_64_GOTPCRELX
                            | R_X86_64_REX_GOTPCRELX,
                        width: 4,
                        ..
                    }
                )
            }
            _ => false,
        }
    }

    pub(in crate::link_object) fn is_tls_reference(self, target: LirTargetProfile) -> bool {
        match target.id() {
            scoop_target_data::DARWIN_AARCH64_ID => {
                matches!(self, Self::TlvpLoadPage21 | Self::TlvpLoadPageOffset12)
            }
            scoop_target_data::LINUX_X86_64_GNU_ID | scoop_target_data::LINUX_X86_64_MUSL_ID => {
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
    fn amd64_calls_addresses_and_tls_do_not_substitute_for_each_other() {
        for target in [
            scoop_target_data::LINUX_X86_64_GNU,
            scoop_target_data::LINUX_X86_64_MUSL,
        ] {
            let call = Form::ElfX86_64Rela {
                kind: R_X86_64_PLT32,
                width: 4,
                addend: -4,
            };
            let inside_function = Form::ElfX86_64Rela {
                kind: R_X86_64_PLT32,
                width: 4,
                addend: 0,
            };
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
            assert!(call.is_direct_call(target));
            assert!(!inside_function.is_direct_call(target));
            assert!(data.is_data_address(target));
            assert!(!data.is_tls_reference(target));
            assert!(tls.is_tls_reference(target));
            assert!(!tls.is_data_address(target));
            assert!(!tls.is_direct_call(target));
            assert!(!Form::Branch26.is_direct_call(target));
            assert!(!Form::TlvpLoadPage21.is_tls_reference(target));
            assert!(!call.is_direct_call(scoop_target_data::DARWIN_AARCH64));
            assert!(!tls.is_tls_reference(scoop_target_data::DARWIN_AARCH64));
        }
    }
}
