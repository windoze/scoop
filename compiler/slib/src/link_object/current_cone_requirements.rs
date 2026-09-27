//! Closed classification of current-Cone undefined strong relocations.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    ConeIdentity, GeneratedBridgeAtomId, GeneratedBridgeUnitId, ObjectDefinitionPlanId, OdrMemberId,
};
use scoop_lir::GeneratedBridgePlanSetV1;

use super::{
    CanonicalUndefinedRelocationUseV1, LinkDefinitionOwnerV1, StrongDefinitionOwnerV1,
    StrongRelocationResolutionV1, VerifiedCurrentConeStrongRelocationClosureV1,
};
use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentConeUndefinedRequirementV1 {
    IntraConeStrong { owner: StrongDefinitionOwnerV1 },
    OdrMember { member: OdrMemberId },
    GeneratedBridge { unit: GeneratedBridgeUnitId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentConeUndefinedRequirementUseV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    target_member: SlibMemberId,
    target_definition: ObjectDefinitionPlanId,
    requirement: CurrentConeUndefinedRequirementV1,
}

impl CurrentConeUndefinedRequirementUseV1 {
    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub const fn target_member(&self) -> SlibMemberId {
        self.target_member
    }

    pub const fn target_definition(&self) -> ObjectDefinitionPlanId {
        self.target_definition
    }

    pub const fn requirement(&self) -> CurrentConeUndefinedRequirementV1 {
        self.requirement
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCurrentConeUndefinedRequirementClosureV1 {
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    generated_bridges: GeneratedBridgePlanSetV1,
    requirements: Vec<CurrentConeUndefinedRequirementUseV1>,
}

impl VerifiedCurrentConeUndefinedRequirementClosureV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.strong_closure.producer()
    }

    pub const fn strong_closure(&self) -> &VerifiedCurrentConeStrongRelocationClosureV1 {
        &self.strong_closure
    }

    pub const fn generated_bridges(&self) -> &GeneratedBridgePlanSetV1 {
        &self.generated_bridges
    }

    pub fn requirements(&self) -> &[CurrentConeUndefinedRequirementUseV1] {
        &self.requirements
    }
}

pub fn verify_current_cone_undefined_requirements_v1(
    strong_closure: VerifiedCurrentConeStrongRelocationClosureV1,
    generated_bridges: GeneratedBridgePlanSetV1,
) -> Result<
    VerifiedCurrentConeUndefinedRequirementClosureV1,
    CurrentConeUndefinedRequirementValidationError,
> {
    if strong_closure.producer() != generated_bridges.producer() {
        return Err(
            CurrentConeUndefinedRequirementValidationError::ProducerMismatch {
                object: strong_closure.producer(),
                bridge: generated_bridges.producer(),
            },
        );
    }

    let mut bridge_atoms = BTreeMap::new();
    for unit in generated_bridges.units() {
        bridge_atoms.insert(unit.primary_atom(), (unit.unit(), true));
        for atom in unit
            .materialized_associated_atoms()
            .chain(unit.static_assert_atoms())
        {
            bridge_atoms.insert(atom, (unit.unit(), false));
        }
    }

    let mut requirements = Vec::new();
    for binding in strong_closure.bindings() {
        let StrongRelocationResolutionV1::CurrentConeUndefinedStrong {
            target_member,
            definition,
            owner,
        } = binding.resolution()
        else {
            continue;
        };
        let requirement = match owner {
            LinkDefinitionOwnerV1::StrongDefinition(owner) => {
                CurrentConeUndefinedRequirementV1::IntraConeStrong { owner }
            }
            LinkDefinitionOwnerV1::OdrDefinition(member) => {
                CurrentConeUndefinedRequirementV1::OdrMember { member }
            }
            LinkDefinitionOwnerV1::GeneratedBridge(atom) => {
                let Some((unit, is_primary)) = bridge_atoms.get(&atom).copied() else {
                    return Err(
                        CurrentConeUndefinedRequirementValidationError::UnplannedGeneratedBridgeAtom {
                            atom,
                        },
                    );
                };
                if !is_primary {
                    return Err(
                        CurrentConeUndefinedRequirementValidationError::NonPrimaryGeneratedBridgeTarget {
                            atom,
                            unit,
                        },
                    );
                }
                CurrentConeUndefinedRequirementV1::GeneratedBridge { unit }
            }
            LinkDefinitionOwnerV1::ConeImage(cone) => {
                return Err(
                    CurrentConeUndefinedRequirementValidationError::ConeImageRelocationTarget {
                        cone,
                    },
                );
            }
            LinkDefinitionOwnerV1::VerifierBoundary { atom, boundary } => {
                return Err(
                    CurrentConeUndefinedRequirementValidationError::VerifierBoundaryRelocationTarget {
                        atom,
                        boundary,
                    },
                );
            }
        };
        requirements.push(CurrentConeUndefinedRequirementUseV1 {
            use_site: CanonicalUndefinedRelocationUseV1::from(binding),
            target_member,
            target_definition: definition,
            requirement,
        });
    }

    Ok(VerifiedCurrentConeUndefinedRequirementClosureV1 {
        strong_closure,
        generated_bridges,
        requirements,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentConeUndefinedRequirementValidationError {
    ProducerMismatch {
        object: ConeIdentity,
        bridge: ConeIdentity,
    },
    UnplannedGeneratedBridgeAtom {
        atom: GeneratedBridgeAtomId,
    },
    NonPrimaryGeneratedBridgeTarget {
        atom: GeneratedBridgeAtomId,
        unit: GeneratedBridgeUnitId,
    },
    ConeImageRelocationTarget {
        cone: ConeIdentity,
    },
    VerifierBoundaryRelocationTarget {
        atom: scoop_identity::ObjectDefinitionAtomId,
        boundary: super::VerifiedBoundaryRoleV1,
    },
}

impl fmt::Display for CurrentConeUndefinedRequirementValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid current-Cone undefined requirement closure: {self:?}"
        )
    }
}

impl std::error::Error for CurrentConeUndefinedRequirementValidationError {}

#[cfg(test)]
pub(super) mod tests;
