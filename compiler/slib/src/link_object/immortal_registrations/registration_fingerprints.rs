use std::fmt;

use scoop_identity::PersistentImmortalObjectId;
use scoop_wire::{HashError, RuntimeEncodeError, RuntimeEncoder};

use super::VerifiedStrongImmortalObjectRegistrationSetV1;
use crate::link_object::{OdrMemberAbiV1, RegistrationAbiV1};

const IMMORTAL_OBJECT_REGISTRATION_RECORD_KIND: u32 = 2;
const OWN_IMMORTAL_ATOM_ROLE: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmortalObjectDefinitionAbiV1 {
    Strong,
    Odr(OdrMemberAbiV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedStrongImmortalObjectFingerprintV1 {
    object: PersistentImmortalObjectId,

    definition: ImmortalObjectDefinitionAbiV1,

    registration: RegistrationAbiV1,
}

impl VerifiedStrongImmortalObjectFingerprintV1 {
    pub const fn object(self) -> PersistentImmortalObjectId {
        self.object
    }

    pub const fn definition(self) -> ImmortalObjectDefinitionAbiV1 {
        self.definition
    }

    pub const fn registration(self) -> RegistrationAbiV1 {
        self.registration
    }
}

/// Shared object and registration ABIs bound to verified immortal records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStrongImmortalObjectFingerprintSetV1 {
    registrations: VerifiedStrongImmortalObjectRegistrationSetV1,
    fingerprints: Vec<VerifiedStrongImmortalObjectFingerprintV1>,
}

impl VerifiedStrongImmortalObjectFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.registrations.producer()
    }

    pub const fn registrations(&self) -> &VerifiedStrongImmortalObjectRegistrationSetV1 {
        &self.registrations
    }

    pub fn fingerprints(&self) -> &[VerifiedStrongImmortalObjectFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_immortal_object_fingerprints_v1(
    registrations: VerifiedStrongImmortalObjectRegistrationSetV1,
    canonical: &scoop_lir::CanonicalShapeAbisV1,
) -> Result<VerifiedStrongImmortalObjectFingerprintSetV1, StrongImmortalObjectFingerprintError> {
    let verified = registrations.registrations();
    let planned = registrations.plan().registrations();
    if verified.len() != planned.len() {
        return Err(StrongImmortalObjectFingerprintError::ProofCoverageMismatch);
    }

    let mut fingerprints = Vec::with_capacity(verified.len());
    for plan in planned {
        let object = plan.object();
        let definition = match (plan.object_definition_owner(), plan.definition_owner()) {
            (
                scoop_identity::ObjectDefinitionPlanOwner::Strong { .. },
                scoop_lir::RegistrationDefinitionOwner::Strong,
            ) => ImmortalObjectDefinitionAbiV1::Strong,
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
                ImmortalObjectDefinitionAbiV1::Odr(
                    crate::link_object::odr_member_fingerprints::shape_definition(*content),
                )
            }
            _ => return Err(StrongImmortalObjectFingerprintError::DefinitionMismatch { object }),
        };
        let registration = RegistrationAbiV1::from_owner(
            plan.definition_owner(),
            scoop_lir::RegistrationTableV1::ImmortalObject,
        )
        .map_err(|source| StrongImmortalObjectFingerprintError::Hash { object, source })?;
        fingerprints.push(VerifiedStrongImmortalObjectFingerprintV1 {
            object,

            definition,

            registration,
        });
    }

    Ok(VerifiedStrongImmortalObjectFingerprintSetV1 {
        registrations,
        fingerprints,
    })
}

pub(in crate::link_object) fn runtime_encode_strong_immortal_object_record_v1(
    encoder: &mut RuntimeEncoder,
    object: PersistentImmortalObjectId,
    object_size: u64,
    required_alignment: u64,
    type_registration: scoop_identity::PersistentExactTypeId,
    owner: scoop_lir::RegistrationDefinitionOwner,
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(IMMORTAL_OBJECT_REGISTRATION_RECORD_KIND)?;
    crate::link_object::registration_identity::runtime_encode_registration_identity(
        encoder,
        object.as_array(),
        owner,
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
