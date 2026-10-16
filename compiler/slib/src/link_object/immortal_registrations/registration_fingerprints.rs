use std::fmt;

use scoop_identity::PersistentImmortalObjectId;
use scoop_wire::{HashError, RuntimeEncodeError, RuntimeEncoder};

use crate::link_object::{OdrMemberAbiV1, RegistrationAbiV1};
use scoop_lir::StrongImmortalObjectRegistrationPlanSetV1;

const IMMORTAL_OBJECT_REGISTRATION_RECORD_KIND: u32 = 2;
const OWN_IMMORTAL_ATOM_ROLE: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmortalObjectDefinitionAbiV1 {
    Strong,
    Odr(OdrMemberAbiV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImmortalObjectFingerprintV1 {
    object: PersistentImmortalObjectId,

    definition: ImmortalObjectDefinitionAbiV1,

    registration: RegistrationAbiV1,
}

impl ImmortalObjectFingerprintV1 {
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

/// Object and registration ABIs computed from the generated registration plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImmortalObjectFingerprintSetV1 {
    plan: StrongImmortalObjectRegistrationPlanSetV1,
    fingerprints: Vec<ImmortalObjectFingerprintV1>,
}

impl ImmortalObjectFingerprintSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.producer()
    }

    pub const fn plan(&self) -> &StrongImmortalObjectRegistrationPlanSetV1 {
        &self.plan
    }

    pub fn fingerprints(&self) -> &[ImmortalObjectFingerprintV1] {
        &self.fingerprints
    }
}

pub fn compute_strong_immortal_object_fingerprints_v1(
    plan: StrongImmortalObjectRegistrationPlanSetV1,
    canonical: &scoop_lir::CanonicalShapeAbisV1,
) -> Result<ImmortalObjectFingerprintSetV1, StrongImmortalObjectFingerprintError> {
    let mut fingerprints = Vec::with_capacity(plan.registrations().len());
    for plan in plan.registrations() {
        let object = plan.object();
        let definition = match plan.object_definition_owner() {
            scoop_identity::ObjectDefinitionPlanOwner::Strong { .. } => {
                ImmortalObjectDefinitionAbiV1::Strong
            }
            scoop_identity::ObjectDefinitionPlanOwner::Odr { member } => {
                let content = canonical.get(member).ok_or(
                    StrongImmortalObjectFingerprintError::MissingCanonicalDefinition { member },
                )?;
                ImmortalObjectDefinitionAbiV1::Odr(
                    crate::link_object::odr_member_fingerprints::shape_definition(*content),
                )
            }
        };
        let registration = RegistrationAbiV1::from_owner(
            plan.definition_owner(),
            scoop_lir::RegistrationTableV1::ImmortalObject,
        )
        .map_err(|source| StrongImmortalObjectFingerprintError::Hash { object, source })?;
        fingerprints.push(ImmortalObjectFingerprintV1 {
            object,

            definition,

            registration,
        });
    }

    Ok(ImmortalObjectFingerprintSetV1 { plan, fingerprints })
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
    MissingCanonicalDefinition {
        member: scoop_identity::OdrMemberId,
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
