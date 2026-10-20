//! Final old/new undefined-use partitions for the cross-Cone profile.

use super::{
    CanonicalUndefinedRelocationUseV1, CanonicalUndefinedSymbolRequirementSetV1,
    FinalUndefinedSymbolRequirementV1, finalize_undefined_symbol_requirements_v1,
};
use crate::link_object::{
    VerifiedCrossConeStrongRequirementClosureV1, VerifiedCurrentConeUndefinedRequirementClosureV1,
    VerifiedRuntimeAndEhRequirementClosureV1,
};
use scoop_identity::{ConeIdentity, StrongCallableDefinitionOwner};
use scoop_lir::ExternalStrongShapeSubjectV1;

mod layout;
pub use layout::*;

/// Mutually exclusive legacy and ordinary-dependency relocation closures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedUndefinedSymbolRequirementPartitionsV1 {
    legacy: CanonicalUndefinedSymbolRequirementSetV1,
    cross_cone: VerifiedCrossConeStrongRequirementClosureV1,
}

/// Requirements used while normalizing object-definition relocations, including
/// the dependency partitions for the selected profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedObjectDefinitionRequirementSetV1 {
    mode: ObjectDefinitionRequirementModeV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ObjectDefinitionRequirementModeV1 {
    SingleCone(CanonicalUndefinedSymbolRequirementSetV1),
    CrossCone(Box<FinalizedUndefinedSymbolRequirementPartitionsV1>),
    Layout(Box<FinalizedLayoutUndefinedSymbolRequirementPartitionsV1>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::link_object) enum CanonicalObjectDefinitionRequirementV1 {
    Legacy(FinalUndefinedSymbolRequirementV1),
    DependencyStrong {
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
    },
    DependencyShapeStrong {
        provider: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
    },
}

impl VerifiedObjectDefinitionRequirementSetV1 {
    pub const fn legacy(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        match &self.mode {
            ObjectDefinitionRequirementModeV1::SingleCone(requirements) => requirements,
            ObjectDefinitionRequirementModeV1::CrossCone(partitions) => partitions.legacy(),
            ObjectDefinitionRequirementModeV1::Layout(partitions) => partitions.legacy(),
        }
    }

    pub const fn cross_cone(&self) -> Option<&FinalizedUndefinedSymbolRequirementPartitionsV1> {
        match &self.mode {
            ObjectDefinitionRequirementModeV1::SingleCone(_) => None,
            ObjectDefinitionRequirementModeV1::CrossCone(partitions) => Some(partitions),
            ObjectDefinitionRequirementModeV1::Layout(_) => None,
        }
    }

    pub const fn layout(&self) -> Option<&FinalizedLayoutUndefinedSymbolRequirementPartitionsV1> {
        match &self.mode {
            ObjectDefinitionRequirementModeV1::Layout(partitions) => Some(partitions),
            ObjectDefinitionRequirementModeV1::SingleCone(_)
            | ObjectDefinitionRequirementModeV1::CrossCone(_) => None,
        }
    }

    pub(in crate::link_object) fn requirement_for(
        &self,
        use_site: &CanonicalUndefinedRelocationUseV1,
    ) -> Option<CanonicalObjectDefinitionRequirementV1> {
        let key = super::use_key(use_site);
        let legacy = self.legacy().requirements();
        if let Some(requirement) = legacy
            .binary_search_by_key(&key, |requirement| super::use_key(requirement.use_site()))
            .ok()
            .map(|index| &legacy[index])
            .filter(|requirement| requirement.use_site() == use_site)
        {
            return Some(CanonicalObjectDefinitionRequirementV1::Legacy(
                requirement.requirement(),
            ));
        }
        let cross_cone = match &self.mode {
            ObjectDefinitionRequirementModeV1::SingleCone(_) => return None,
            ObjectDefinitionRequirementModeV1::CrossCone(partitions) => partitions.cross_cone(),
            ObjectDefinitionRequirementModeV1::Layout(partitions) => partitions.cross_cone(),
        };
        let requirements = cross_cone.requirements();
        if let Some(requirement) = requirements
            .binary_search_by_key(&key, |requirement| super::use_key(requirement.use_site()))
            .ok()
            .map(|index| &requirements[index])
            .filter(|requirement| requirement.use_site() == use_site)
        {
            let import = cross_cone
                .semantic_imports()
                .imports()
                .get(requirement.import_index() as usize)?;
            return Some(CanonicalObjectDefinitionRequirementV1::DependencyStrong {
                provider: import.provider(),
                target: import.target(),
            });
        }
        match &self.mode {
            ObjectDefinitionRequirementModeV1::Layout(partitions) => {
                partitions.shape_requirement_for(use_site)
            }
            ObjectDefinitionRequirementModeV1::SingleCone(_)
            | ObjectDefinitionRequirementModeV1::CrossCone(_) => None,
        }
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
}

pub fn finalize_partitioned_undefined_symbol_requirements_v1(
    current_cone: VerifiedCurrentConeUndefinedRequirementClosureV1,
    external: VerifiedRuntimeAndEhRequirementClosureV1,
) -> FinalizedUndefinedSymbolRequirementPartitionsV1 {
    let cross_cone = external.source_closure().cross_cone_closure().clone();
    let legacy = finalize_undefined_symbol_requirements_v1(current_cone, external);
    FinalizedUndefinedSymbolRequirementPartitionsV1 { legacy, cross_cone }
}
