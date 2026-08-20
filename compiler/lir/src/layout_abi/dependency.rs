use super::*;
use scoop_identity::PersistentIdResolver;

/// A terminal provider and one semantic layout/ABI record requested from it.
/// This relation is transport data, not a branded selected handle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LayoutAbiDependencyV1 {
    pub(super) provider: ConeIdentity,
    pub(super) target: LayoutAbiSemanticTargetV1,
}

impl LayoutAbiDependencyV1 {
    pub const fn new(provider: ConeIdentity, target: LayoutAbiSemanticTargetV1) -> Self {
        Self { provider, target }
    }

    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }

    pub const fn target(self) -> LayoutAbiSemanticTargetV1 {
        self.target
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedLayoutAbiDependencyV1 {
    pub(super) provider: DecodedPersistentId<ConeIdentity>,
    pub(super) target: DecodedLayoutAbiSemanticTargetV1,
}

impl DecodedLayoutAbiDependencyV1 {
    pub fn resolve(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<LayoutAbiDependencyV1, LayoutAbiDependencyError> {
        Ok(LayoutAbiDependencyV1::new(
            identities.resolve(self.provider)?,
            self.target.resolve(identities)?,
        ))
    }
}
