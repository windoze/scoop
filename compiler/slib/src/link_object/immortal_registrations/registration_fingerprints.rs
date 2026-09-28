use std::fmt;

use scoop_identity::{DigestKind, DigestNodeId, PersistentImmortalObjectId};
use scoop_wire::{
    HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, domain_separated_runtime_hash,
};

use super::VerifiedStrongImmortalObjectDefinitionFingerprintSetV1;
use crate::link_object::callable_registrations::object_definition::CanonicalDigestInputV1;
use crate::link_object::{
    ObjectDefinitionFingerprintV1, OdrMemberFingerprintV1, RegistrationFingerprintV1,
    StrongRegistrationFingerprintV1,
};

const STRONG_REGISTRATION_DOMAIN: &str = "scoop-strong-registration-v1";
const IMMORTAL_OBJECT_REGISTRATION_RECORD_KIND: u32 = 2;
const OWN_IMMORTAL_ATOM_ROLE: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmortalObjectDefinitionFingerprintV1 {
    Strong(ObjectDefinitionFingerprintV1),
    Odr(OdrMemberFingerprintV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongImmortalObjectFingerprintV1 {
    object: PersistentImmortalObjectId,
    registration_object_node: DigestNodeId,
    registration_object: ObjectDefinitionFingerprintV1,
    object_definition_node: DigestNodeId,
    object_definition: ObjectDefinitionFingerprintV1,
    definition: ImmortalObjectDefinitionFingerprintV1,
    registration_node: DigestNodeId,
    registration: RegistrationFingerprintV1,
}

impl VerifiedStrongImmortalObjectFingerprintV1 {
    pub const fn object(self) -> PersistentImmortalObjectId {
        self.object
    }

    pub const fn registration_object_node(self) -> DigestNodeId {
        self.registration_object_node
    }

    pub const fn registration_object(self) -> ObjectDefinitionFingerprintV1 {
        self.registration_object
    }

    pub const fn object_definition_node(self) -> DigestNodeId {
        self.object_definition_node
    }

    pub const fn object_definition(self) -> ObjectDefinitionFingerprintV1 {
        self.object_definition
    }

    pub const fn registration_node(self) -> DigestNodeId {
        self.registration_node
    }

    pub const fn definition(self) -> ImmortalObjectDefinitionFingerprintV1 {
        self.definition
    }

    pub const fn registration(self) -> RegistrationFingerprintV1 {
        self.registration
    }
}

/// Canonical strong-registration fingerprints derived from both complete
/// immortal-object leaves.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongImmortalObjectFingerprintSetV1 {
    object_definitions: VerifiedStrongImmortalObjectDefinitionFingerprintSetV1,
    fingerprints: Vec<VerifiedStrongImmortalObjectFingerprintV1>,
}

impl VerifiedStrongImmortalObjectFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.object_definitions.producer()
    }

    pub const fn object_definitions(
        &self,
    ) -> &VerifiedStrongImmortalObjectDefinitionFingerprintSetV1 {
        &self.object_definitions
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongImmortalObjectFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_immortal_object_fingerprints_v1(
    object_definitions: VerifiedStrongImmortalObjectDefinitionFingerprintSetV1,
    canonical: &scoop_lir::CanonicalShapeLirDefinitionsV1,
) -> Result<VerifiedStrongImmortalObjectFingerprintSetV1, StrongImmortalObjectFingerprintError> {
    let registration_objects = object_definitions.registration_objects();
    let registrations = registration_objects.registrations();
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    let registration_fingerprints = registration_objects.fingerprints();
    let object_fingerprints = object_definitions.fingerprints();
    if verified.len() != planned.len()
        || verified.len() != registration_fingerprints.len()
        || verified.len() != object_fingerprints.len()
    {
        return Err(StrongImmortalObjectFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for (((verified, plan), registration_object), object_definition) in verified
        .iter()
        .zip(planned)
        .zip(registration_fingerprints)
        .zip(object_fingerprints)
    {
        let object = plan.object();
        if verified.object() != object
            || registration_object.object() != object
            || registration_object.node() != plan.registration_object_node()
        {
            return Err(
                StrongImmortalObjectFingerprintError::RegistrationObjectMismatch { object },
            );
        }
        if object_definition.object() != object
            || object_definition.node() != plan.object_definition_node()
        {
            return Err(StrongImmortalObjectFingerprintError::ObjectDefinitionMismatch { object });
        }
        let definition = match (plan.object_definition_owner(), plan.definition_owner()) {
            (
                scoop_identity::ObjectDefinitionPlanOwner::Strong { .. },
                scoop_lir::RegistrationDefinitionOwner::Strong,
            ) => ImmortalObjectDefinitionFingerprintV1::Strong(object_definition.fingerprint()),
            (
                scoop_identity::ObjectDefinitionPlanOwner::Odr { member },
                scoop_lir::RegistrationDefinitionOwner::Odr { group, .. },
            ) => {
                let content = canonical
                    .get(member)
                    .filter(|content| {
                        content.group() == group
                            && content.role() == scoop_identity::OdrMemberRole::ImmortalObject
                            && content.definition() == plan.object_definition_plan()
                            && content.primary_atom() == plan.object_primary_atom()
                            && content.entity()
                                == scoop_identity::StrongDefinitionEntity::immortal_object(object)
                    })
                    .ok_or(StrongImmortalObjectFingerprintError::DefinitionMismatch { object })?;
                ImmortalObjectDefinitionFingerprintV1::Odr(
                    crate::link_object::odr_member_fingerprints::shape_definition(
                        *content,
                        object_definition.fingerprint(),
                    )
                    .map_err(|source| {
                        StrongImmortalObjectFingerprintError::Hash { object, source }
                    })?,
                )
            }
            _ => return Err(StrongImmortalObjectFingerprintError::DefinitionMismatch { object }),
        };
        let registration = match plan.definition_owner() {
            scoop_lir::RegistrationDefinitionOwner::Strong => {
                strong_immortal_object_registration_fingerprint(
                    *plan,
                    registration_object.node(),
                    registration_object.fingerprint(),
                    object_definition.node(),
                    object_definition.fingerprint(),
                )
                .map(RegistrationFingerprintV1::Strong)
            }
            scoop_lir::RegistrationDefinitionOwner::Odr { group, member } => {
                crate::link_object::odr_member_fingerprints::immortal_registration(
                    group,
                    member,
                    *plan,
                    registration_object.fingerprint(),
                    object_definition.fingerprint(),
                )
                .map(RegistrationFingerprintV1::Odr)
            }
        }
        .map_err(|source| StrongImmortalObjectFingerprintError::Hash { object, source })?;
        fingerprints.push(VerifiedStrongImmortalObjectFingerprintV1 {
            object,
            registration_object_node: registration_object.node(),
            registration_object: registration_object.fingerprint(),
            object_definition_node: object_definition.node(),
            object_definition: object_definition.fingerprint(),
            definition,
            registration_node: plan.registration_fingerprint_node(),
            registration,
        });
    }

    Ok(VerifiedStrongImmortalObjectFingerprintSetV1 {
        object_definitions,
        fingerprints,
    })
}

fn strong_immortal_object_registration_fingerprint(
    plan: scoop_lir::StrongImmortalObjectRegistrationPlanV1,
    registration_object_node: DigestNodeId,
    registration_object: ObjectDefinitionFingerprintV1,
    object_definition_node: DigestNodeId,
    object_definition: ObjectDefinitionFingerprintV1,
) -> Result<StrongRegistrationFingerprintV1, HashError> {
    let mut direct_inputs = [
        CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: registration_object_node,
            digest: *registration_object.as_array(),
        },
        CanonicalDigestInputV1 {
            kind: DigestKind::ObjectDefinition,
            node: object_definition_node,
            digest: *object_definition.as_array(),
        },
    ];
    direct_inputs.sort_unstable_by_key(|input| (input.kind.tag(), input.node));
    domain_separated_runtime_hash(
        STRONG_REGISTRATION_DOMAIN,
        &StrongImmortalObjectRegistrationFingerprintInputV1 {
            object: plan.object(),
            object_size: plan.object_size(),
            required_alignment: plan.required_alignment(),
            type_registration: plan.type_registration(),
            direct_inputs,
        },
    )
    .map(|digest| StrongRegistrationFingerprintV1::from_array(*digest.as_array()))
}

struct StrongImmortalObjectRegistrationFingerprintInputV1 {
    object: PersistentImmortalObjectId,
    object_size: u64,
    required_alignment: u64,
    type_registration: scoop_identity::PersistentExactTypeId,
    direct_inputs: [CanonicalDigestInputV1; 2],
}

impl RuntimeEncode for StrongImmortalObjectRegistrationFingerprintInputV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        runtime_encode_strong_immortal_object_record_v1(
            encoder,
            self.object,
            self.object_size,
            self.required_alignment,
            self.type_registration,
            scoop_lir::RegistrationDefinitionOwner::Strong,
            &[0; 32],
        )?;
        encoder.sequence_length(self.direct_inputs.len())?;
        for input in self.direct_inputs {
            input.runtime_encode(encoder)?;
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::link_object) fn runtime_encode_strong_immortal_object_record_v1(
    encoder: &mut RuntimeEncoder,
    object: PersistentImmortalObjectId,
    object_size: u64,
    required_alignment: u64,
    type_registration: scoop_identity::PersistentExactTypeId,
    owner: scoop_lir::RegistrationDefinitionOwner,
    registration: &[u8; 32],
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(IMMORTAL_OBJECT_REGISTRATION_RECORD_KIND)?;
    crate::link_object::registration_identity::runtime_encode_registration_identity(
        encoder,
        object.as_array(),
        owner,
        registration,
    )?;
    encoder.u32(OWN_IMMORTAL_ATOM_ROLE)?;
    encoder.fixed(object.as_array())?;
    encoder.u64(object_size)?;
    encoder.u64(required_alignment)?;
    encoder.fixed(type_registration.as_array())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongImmortalObjectFingerprintError {
    DefinitionMismatch {
        object: PersistentImmortalObjectId,
    },
    ProofCoverageMismatch,
    RegistrationObjectMismatch {
        object: PersistentImmortalObjectId,
    },
    ObjectDefinitionMismatch {
        object: PersistentImmortalObjectId,
    },
    Hash {
        object: PersistentImmortalObjectId,
        source: HashError,
    },
}

impl fmt::Display for StrongImmortalObjectFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute strong immortal-object fingerprints: {self:?}"
        )
    }
}

impl std::error::Error for StrongImmortalObjectFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
