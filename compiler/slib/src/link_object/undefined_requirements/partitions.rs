//! Final old/new undefined-use partitions for the cross-Cone profile.

use super::{
    CanonicalUndefinedRelocationUseV1, CanonicalUndefinedSymbolRequirementSetV1,
    FinalUndefinedSymbolRequirementV1, StrongRelocationResolutionV1,
    UndefinedSymbolRequirementFinalizationError,
    finalize_partitioned_undefined_symbol_requirements_inner, use_key,
};
use crate::link_object::{
    SealedBuiltinObjectExternalRequirementClosureV1, VerifiedCrossConeStrongRequirementClosureV1,
    VerifiedCurrentConeStrongRelocationClosureV1, VerifiedCurrentConeUndefinedRequirementClosureV1,
};
use scoop_identity::{ConeIdentity, StrongCallableDefinitionOwner};

/// Mutually exclusive legacy and ordinary-dependency relocation closures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedUndefinedSymbolRequirementPartitionsV1 {
    legacy: CanonicalUndefinedSymbolRequirementSetV1,
    cross_cone: VerifiedCrossConeStrongRequirementClosureV1,
}

/// Complete requirement authority used while normalizing object-definition
/// relocations. The profile mode is retained so a cross-Cone proof cannot be
/// silently collapsed to its legacy partition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedObjectDefinitionRequirementSetV1 {
    mode: ObjectDefinitionRequirementModeV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ObjectDefinitionRequirementModeV1 {
    SingleCone(CanonicalUndefinedSymbolRequirementSetV1),
    CrossCone(Box<FinalizedUndefinedSymbolRequirementPartitionsV1>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::link_object) enum CanonicalObjectDefinitionRequirementV1 {
    Legacy(FinalUndefinedSymbolRequirementV1),
    DependencyStrong {
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
    },
}

impl VerifiedObjectDefinitionRequirementSetV1 {
    pub const fn legacy(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        match &self.mode {
            ObjectDefinitionRequirementModeV1::SingleCone(requirements) => requirements,
            ObjectDefinitionRequirementModeV1::CrossCone(partitions) => partitions.legacy(),
        }
    }

    pub const fn cross_cone(&self) -> Option<&FinalizedUndefinedSymbolRequirementPartitionsV1> {
        match &self.mode {
            ObjectDefinitionRequirementModeV1::SingleCone(_) => None,
            ObjectDefinitionRequirementModeV1::CrossCone(partitions) => Some(partitions),
        }
    }

    pub(in crate::link_object) fn matches_strong_closure(
        &self,
        closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    ) -> bool {
        match &self.mode {
            ObjectDefinitionRequirementModeV1::SingleCone(requirements) => {
                requirements.matches_strong_closure(closure)
            }
            ObjectDefinitionRequirementModeV1::CrossCone(partitions) => {
                partitions.matches_strong_closure(closure)
            }
        }
    }

    pub(in crate::link_object) fn requirement_for(
        &self,
        use_site: &CanonicalUndefinedRelocationUseV1,
    ) -> Option<CanonicalObjectDefinitionRequirementV1> {
        if let Some(requirement) = self
            .legacy()
            .requirements()
            .iter()
            .find(|requirement| requirement.use_site() == use_site)
        {
            return Some(CanonicalObjectDefinitionRequirementV1::Legacy(
                requirement.requirement(),
            ));
        }
        let partitions = self.cross_cone()?;
        let requirement = partitions
            .cross_cone()
            .requirements()
            .iter()
            .find(|requirement| requirement.use_site() == use_site)?;
        let import = partitions
            .cross_cone()
            .semantic_imports()
            .imports()
            .get(requirement.import_index() as usize)?;
        Some(CanonicalObjectDefinitionRequirementV1::DependencyStrong {
            provider: import.provider(),
            target: import.target(),
        })
    }
}

impl From<CanonicalUndefinedSymbolRequirementSetV1> for VerifiedObjectDefinitionRequirementSetV1 {
    fn from(requirements: CanonicalUndefinedSymbolRequirementSetV1) -> Self {
        Self {
            mode: ObjectDefinitionRequirementModeV1::SingleCone(requirements),
        }
    }
}

impl From<FinalizedUndefinedSymbolRequirementPartitionsV1>
    for VerifiedObjectDefinitionRequirementSetV1
{
    fn from(partitions: FinalizedUndefinedSymbolRequirementPartitionsV1) -> Self {
        Self {
            mode: ObjectDefinitionRequirementModeV1::CrossCone(Box::new(partitions)),
        }
    }
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
        closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    ) -> bool {
        if self.legacy.producer() != closure.producer()
            || self.cross_cone.producer() != closure.producer()
            || self.legacy.selection()
                != scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
            || self.cross_cone.target() != scoop_lir::LirTargetProfile::DARWIN_AARCH64
        {
            return false;
        }

        let mut expected = closure
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
