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

    pub const fn target(self) -> crate::LirTargetProfile {
        self.target
    }

    pub const fn backend(self) -> crate::BackendProfile {
        self.backend
    }
}
