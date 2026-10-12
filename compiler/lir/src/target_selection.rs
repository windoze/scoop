#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ValidatedLirTargetSelection {
    target: crate::LirTargetProfile,
    backend: crate::BackendProfile,
}

impl ValidatedLirTargetSelection {
    pub const fn new(target: crate::LirTargetProfile, backend: crate::BackendProfile) -> Self {
        Self { target, backend }
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

    use crate::NativeSymbolNormalization;

    #[test]
    fn linux_libcs_have_distinct_targets_and_share_the_x86_backend() {
        let gnu = crate::target_fixtures::selection(crate::target_fixtures::LINUX_X86_64_GNU_ID)
            .expect("implemented target selected by this consumer");
        let musl = crate::target_fixtures::selection(crate::target_fixtures::LINUX_X86_64_MUSL_ID)
            .expect("implemented target selected by this consumer");
        assert_eq!(gnu.backend(), musl.backend());
        assert_ne!(
            gnu.backend().wire_id(),
            crate::target_fixtures::LLVM_22_1.wire_id()
        );
        assert_ne!(
            gnu.backend().fingerprint().unwrap(),
            crate::target_fixtures::LLVM_22_1.fingerprint().unwrap()
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
                profile.native_object_format().wire_id(),
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
