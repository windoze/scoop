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
