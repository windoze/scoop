//! Foundation, HIR resolution, and legacy-production type-state transitions.

use scoop_identity::ConeIdentity;
use scoop_lir::ValidatedLirTargetSelection;

use crate::{
    FoundationValidatedCrossConeHirFrontSections, HirProductionValidatedCrossConeHirFrontSections,
    ResolvedCrossConeHirFrontSections,
};

use super::{
    CrossConeClosureHirProductionError, CrossConeClosureHirResolutionError, CrossConeProviderRole,
    FoundationValidatedCrossConeHirClosure, HirProductionValidatedCrossConeHirClosure,
    ResolvedCrossConeHirClosure,
};

impl<'input> FoundationValidatedCrossConeHirClosure<'input> {
    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        &self.direct
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &FoundationValidatedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&FoundationValidatedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
        self.positions.get(&identity).map(|_| {
            if self.direct.binary_search(&identity).is_ok() {
                CrossConeProviderRole::Direct
            } else {
                CrossConeProviderRole::Support
            }
        })
    }

    pub fn dependency_count(&self, identity: ConeIdentity) -> Option<usize> {
        self.positions
            .get(&identity)
            .map(|position| self.dependency_positions[*position].len())
    }

    /// Resolves every general HIR section against the same identity graph
    /// that proved its foundation. This does not yet grant public-surface or
    /// route authority.
    pub fn resolve_hir_interfaces(
        self,
    ) -> Result<ResolvedCrossConeHirClosure<'input>, CrossConeClosureHirResolutionError> {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut resolved = Vec::new();
        resolved.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureHirResolutionError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for front in dependency_first {
            let identity = front.identity();
            resolved.push(front.resolve_hir_interface().map_err(|source| {
                CrossConeClosureHirResolutionError::Artifact {
                    identity,
                    source: Box::new(source),
                }
            })?);
        }

        Ok(ResolvedCrossConeHirClosure {
            current,
            target,
            direct,
            dependency_first: resolved,
            positions,
            dependency_positions,
        })
    }
}

impl ResolvedCrossConeHirClosure<'_> {
    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        &self.direct
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &ResolvedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&ResolvedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
        self.positions.get(&identity).map(|_| {
            if self.direct.binary_search(&identity).is_ok() {
                CrossConeProviderRole::Direct
            } else {
                CrossConeProviderRole::Support
            }
        })
    }

    pub fn dependency_count(&self, identity: ConeIdentity) -> Option<usize> {
        self.positions
            .get(&identity)
            .map(|position| self.dependency_positions[*position].len())
    }
}

impl<'input> ResolvedCrossConeHirClosure<'input> {
    /// Validates every provider's unchanged M23-3 HIR production section
    /// before its direct-public surface is used as M23-5 semantic authority.
    pub fn validate_hir_productions(
        self,
    ) -> Result<HirProductionValidatedCrossConeHirClosure<'input>, CrossConeClosureHirProductionError>
    {
        let Self {
            current,
            target,
            direct,
            dependency_first,
            positions,
            dependency_positions,
        } = self;
        let artifact_count = dependency_first.len();
        let mut validated = Vec::new();
        validated.try_reserve_exact(artifact_count).map_err(|_| {
            CrossConeClosureHirProductionError::Allocation {
                requested_slots: artifact_count,
            }
        })?;
        for front in dependency_first {
            let identity = front.identity();
            validated.push(front.validate_hir_production().map_err(|source| {
                CrossConeClosureHirProductionError::Artifact { identity, source }
            })?);
        }

        Ok(HirProductionValidatedCrossConeHirClosure {
            current,
            target,
            direct,
            dependency_first: validated,
            positions,
            dependency_positions,
        })
    }
}

impl HirProductionValidatedCrossConeHirClosure<'_> {
    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub fn direct_providers(&self) -> &[ConeIdentity] {
        &self.direct
    }

    pub fn dependency_first(
        &self,
    ) -> impl ExactSizeIterator<Item = &HirProductionValidatedCrossConeHirFrontSections<'_>> {
        self.dependency_first.iter()
    }

    pub fn artifact(
        &self,
        identity: ConeIdentity,
    ) -> Option<&HirProductionValidatedCrossConeHirFrontSections<'_>> {
        self.positions
            .get(&identity)
            .map(|position| &self.dependency_first[*position])
    }

    pub fn role(&self, identity: ConeIdentity) -> Option<CrossConeProviderRole> {
        self.positions.get(&identity).map(|_| {
            if self.direct.binary_search(&identity).is_ok() {
                CrossConeProviderRole::Direct
            } else {
                CrossConeProviderRole::Support
            }
        })
    }

    pub fn dependency_count(&self, identity: ConeIdentity) -> Option<usize> {
        self.positions
            .get(&identity)
            .map(|position| self.dependency_positions[*position].len())
    }
}
