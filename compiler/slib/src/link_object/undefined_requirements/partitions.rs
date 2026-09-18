//! Final old/new undefined-use partitions for the cross-Cone profile.

use super::{
    CanonicalUndefinedRelocationUseV1, CanonicalUndefinedSymbolRequirementSetV1,
    StrongRelocationResolutionV1, UndefinedSymbolRequirementFinalizationError,
    finalize_partitioned_undefined_symbol_requirements_inner, use_key,
};
use crate::link_object::{
    SealedBuiltinObjectExternalRequirementClosureV1, VerifiedBuiltinObjectStrongRelocationSetV1,
    VerifiedCrossConeStrongRequirementClosureV1, VerifiedCurrentConeUndefinedRequirementClosureV1,
};

/// Mutually exclusive legacy and ordinary-dependency relocation closures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedUndefinedSymbolRequirementPartitionsV1 {
    legacy: CanonicalUndefinedSymbolRequirementSetV1,
    cross_cone: VerifiedCrossConeStrongRequirementClosureV1,
}

impl FinalizedUndefinedSymbolRequirementPartitionsV1 {
    pub const fn legacy(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.legacy
    }

    pub const fn cross_cone(&self) -> &VerifiedCrossConeStrongRequirementClosureV1 {
        &self.cross_cone
    }

    pub fn into_parts(
        self,
    ) -> (
        CanonicalUndefinedSymbolRequirementSetV1,
        VerifiedCrossConeStrongRequirementClosureV1,
    ) {
        (self.legacy, self.cross_cone)
    }

    pub(in crate::link_object) fn matches_strong_closure(
        &self,
        builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    ) -> bool {
        if self.legacy.producer() != builtins.producer()
            || self.cross_cone.producer() != builtins.producer()
            || self.legacy.selection()
                != scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
            || self.cross_cone.target() != scoop_lir::LirTargetProfile::DARWIN_AARCH64
        {
            return false;
        }

        let mut expected = builtins
            .strong_relocations()
            .bindings()
            .iter()
            .filter(|binding| {
                !matches!(
                    binding.resolution(),
                    StrongRelocationResolutionV1::ObjectLocalStrong { .. }
                )
            })
            .map(CanonicalUndefinedRelocationUseV1::from)
            .collect::<Vec<_>>();
        expected.sort_unstable_by_key(use_key);

        let mut actual = self
            .legacy
            .requirements()
            .iter()
            .map(|requirement| requirement.use_site().clone())
            .chain(
                self.cross_cone
                    .requirements()
                    .iter()
                    .map(|requirement| requirement.use_site().clone()),
            )
            .collect::<Vec<_>>();
        actual.sort_unstable_by_key(use_key);
        actual == expected
    }
}

pub fn finalize_partitioned_undefined_symbol_requirements_v1(
    current_cone: VerifiedCurrentConeUndefinedRequirementClosureV1,
    external: SealedBuiltinObjectExternalRequirementClosureV1,
) -> Result<
    FinalizedUndefinedSymbolRequirementPartitionsV1,
    UndefinedSymbolRequirementFinalizationError,
> {
    let cross_cone = external.cross_cone_closure().clone();
    let legacy = finalize_partitioned_undefined_symbol_requirements_inner(current_cone, external)?;
    Ok(FinalizedUndefinedSymbolRequirementPartitionsV1 { legacy, cross_cone })
}
