#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ValidatedLirTargetSelection {
    target: crate::LirTargetProfile,
    backend: crate::BackendProfile,
}

impl ValidatedLirTargetSelection {
    pub const DARWIN_AARCH64_LLVM_22_1: Self = Self {
        target: crate::LirTargetProfile::DARWIN_AARCH64,
        backend: crate::BackendProfile::LLVM_22_1,
    };

    pub const LINUX_X86_64_GNU_LLVM_22_1: Self = Self {
        target: crate::LirTargetProfile::LINUX_X86_64_GNU,
        backend: crate::BackendProfile::LLVM_22_1_LINUX_X86_64,
    };
    pub const LINUX_X86_64_MUSL_LLVM_22_1: Self = Self {
        target: crate::LirTargetProfile::LINUX_X86_64_MUSL,
        backend: crate::BackendProfile::LLVM_22_1_LINUX_X86_64,
    };

    pub const fn from_id(id: crate::TargetProfileId) -> Self {
        match id {
            crate::TargetProfileId::DarwinAarch64 => Self::DARWIN_AARCH64_LLVM_22_1,
            crate::TargetProfileId::LinuxX86_64Gnu => Self::LINUX_X86_64_GNU_LLVM_22_1,
            crate::TargetProfileId::LinuxX86_64Musl => Self::LINUX_X86_64_MUSL_LLVM_22_1,
        }
    }

    pub const fn target(self) -> crate::LirTargetProfile {
        self.target
    }

    pub const fn backend(self) -> crate::BackendProfile {
        self.backend
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackendProfile, NativeSymbolNormalization, TargetProfileId};

    #[test]
    fn linux_libcs_have_distinct_targets_and_share_the_x86_backend() {
        let gnu = ValidatedLirTargetSelection::from_id(TargetProfileId::LinuxX86_64Gnu);
        let musl = ValidatedLirTargetSelection::from_id(TargetProfileId::LinuxX86_64Musl);
        assert_eq!(gnu.backend(), musl.backend());
        assert_ne!(gnu.backend().wire_id(), BackendProfile::LLVM_22_1.wire_id());
        assert_ne!(
            gnu.backend().fingerprint().unwrap(),
            BackendProfile::LLVM_22_1.fingerprint().unwrap()
        );
        assert_ne!(gnu.target().wire_id(), musl.target().wire_id());
        assert_ne!(
            gnu.target().fingerprint().unwrap(),
            musl.target().fingerprint().unwrap()
        );
        for profile in [gnu.target(), musl.target()] {
            assert_eq!(
                profile.contract().native_symbol_normalization(),
                NativeSymbolNormalization::ElfIdentity
            );
            assert_eq!(profile.contract().stack_alignment_bytes(), 16);
            assert_eq!(profile.managed_pointer_layout().size_bytes(), 8);
            assert_eq!(
                profile.id().object_format(),
                scoop_identity::ObjectFormatId::elf_relocatable()
            );
            assert_eq!(
                profile
                    .contract()
                    .native_symbol_normalization()
                    .compiler_generated_object_symbol("_entry"),
                "_entry"
            );
        }
    }
}
