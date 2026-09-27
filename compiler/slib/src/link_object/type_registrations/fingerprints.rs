use std::fmt;

use scoop_identity::{DigestKind, PersistentExactTypeId};
use scoop_wire::{HashError, domain_separated_runtime_hash};

use super::VerifiedStrongTypeRegistrationV1;
use super::dependency_fingerprints::VerifiedStrongTypeDependencyFingerprintV1;
use super::record::DESCRIPTOR_SIZE;
use crate::SlibMemberId;
use crate::link_object::callable_registrations::object_definition::{
    CanonicalDigestInputV1, CanonicalObjectRelocationV1, ObjectDefinitionFingerprintInputV1,
    canonicalize_relocations,
};
use crate::link_object::{
    ObjectDefinitionFingerprintV1, ObjectDefinitionRelocationFailureV1,
    VerifiedCurrentConeStrongRelocationClosureV1, VerifiedObjectDefinitionRequirementSetV1,
};

pub(super) fn registration_object<D: Copy, C>(
    plan: &scoop_lir::StrongTypeRegistrationPlan<D, C>,
    verified: &VerifiedStrongTypeRegistrationV1,
    dependency: &VerifiedStrongTypeDependencyFingerprintV1,
    object: &[u8],
    closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    requirements: &VerifiedObjectDefinitionRequirementSetV1,
) -> Result<ObjectDefinitionFingerprintV1, StrongTypeRegistrationObjectFingerprintError> {
    let exact_type = plan.exact_type();
    let start = usize::try_from(verified.checked_offset())
        .map_err(|_| StrongTypeRegistrationObjectFingerprintError::RecordRange(exact_type))?;
    let end = start.checked_add(DESCRIPTOR_SIZE).ok_or(
        StrongTypeRegistrationObjectFingerprintError::RecordRange(exact_type),
    )?;
    let bytes =
        object
            .get(start..end)
            .ok_or(StrongTypeRegistrationObjectFingerprintError::RecordRange(
                exact_type,
            ))?;
    let mut normalized = bytes.to_vec();
    let (relocations, mut direct_inputs) = match plan.definition_owner() {
        scoop_lir::RegistrationDefinitionOwner::Strong => (
            vec![CanonicalObjectRelocationV1::type_descriptor(exact_type)],
            Vec::new(),
        ),
        scoop_lir::RegistrationDefinitionOwner::Odr { .. } => {
            let member = closure
                .members()
                .iter()
                .find(|member| member.member() == verified.member())
                .ok_or(StrongTypeRegistrationObjectFingerprintError::MissingObject(
                    verified.member(),
                ))?;
            let relocations =
                canonicalize_relocations(bytes, member, plan.primary_atom(), closure, requirements)
                    .map_err(
                        |kind| StrongTypeRegistrationObjectFingerprintError::Relocation {
                            exact_type,
                            kind,
                        },
                    )?;
            for relocation in &relocations {
                relocation
                    .normalize_bytes(&mut normalized)
                    .map_err(
                        |kind| StrongTypeRegistrationObjectFingerprintError::Relocation {
                            exact_type,
                            kind,
                        },
                    )?;
            }
            normalized[176..208].copy_from_slice(dependency.descriptor_definition().as_array());
            normalized[208..240].copy_from_slice(dependency.layout().as_array());
            (
                relocations,
                vec![
                    CanonicalDigestInputV1 {
                        kind: DigestKind::ObjectDefinition,
                        node: dependency.descriptor_definition_node(),
                        digest: *dependency.descriptor_definition().as_array(),
                    },
                    CanonicalDigestInputV1 {
                        kind: DigestKind::Layout,
                        node: dependency.layout_node(),
                        digest: *dependency.layout().as_array(),
                    },
                ],
            )
        }
    };
    direct_inputs.sort_unstable_by_key(|input| (input.kind.tag(), input.node));
    domain_separated_runtime_hash(
        "scoop-object-definition-v1",
        &ObjectDefinitionFingerprintInputV1 {
            bytes: &normalized,
            relocations: &relocations,
            direct_inputs: &direct_inputs,
        },
    )
    .map(|digest| ObjectDefinitionFingerprintV1::from_array(*digest.as_array()))
    .map_err(|source| StrongTypeRegistrationObjectFingerprintError::Hash { exact_type, source })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongTypeRegistrationObjectFingerprintError {
    MissingObject(SlibMemberId),
    RecordRange(PersistentExactTypeId),
    Relocation {
        exact_type: PersistentExactTypeId,
        kind: ObjectDefinitionRelocationFailureV1,
    },
    Hash {
        exact_type: PersistentExactTypeId,
        source: HashError,
    },
}

impl fmt::Display for StrongTypeRegistrationObjectFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute type registration object content: {self:?}"
        )
    }
}
impl std::error::Error for StrongTypeRegistrationObjectFingerprintError {}

#[cfg(test)]
mod tests;
