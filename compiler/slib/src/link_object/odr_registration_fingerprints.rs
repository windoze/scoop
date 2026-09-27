//! Member-local fingerprints for actual callable and safepoint registrations.

use std::fmt;

use scoop_identity::{
    DigestKind, DigestNodeId, ObjectDefinitionAtomId, OdrGroupId, OdrMemberId, OdrMemberRole,
    PersistentSafepointSiteId,
};
use scoop_wire::{Digest256, HashError, domain_separated_cbor_hash};

use super::{
    ObjectDefinitionFingerprintV1, OdrAbiFingerprintV1, OdrDefinitionFingerprintV1,
    StackmapRecordFingerprintV1, StrongRegistrationFingerprintV1,
};

mod encode;
use encode::{AbiInput, DefinitionInput, RegistrationProjection};

/// The definition slot has one of two distinct digest owners and algorithms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationFingerprintV1 {
    Strong(StrongRegistrationFingerprintV1),
    Odr(OdrRegistrationFingerprintV1),
}

impl RegistrationFingerprintV1 {
    pub const fn kind(self) -> DigestKind {
        match self {
            Self::Strong(_) => DigestKind::StrongRegistration,
            Self::Odr(_) => DigestKind::OdrDefinition,
        }
    }

    pub const fn as_array(&self) -> &[u8; 32] {
        match self {
            Self::Strong(value) => value.as_array(),
            Self::Odr(value) => value.definition.as_array(),
        }
    }
}

impl fmt::Display for RegistrationFingerprintV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strong(value) => value.fmt(formatter),
            Self::Odr(value) => value.definition.fmt(formatter),
        }
    }
}

/// Content of one registration member, independent of producer and placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OdrRegistrationFingerprintV1 {
    group: OdrGroupId,
    member: OdrMemberId,
    abi: OdrAbiFingerprintV1,
    lir: Digest256,
    definition: OdrDefinitionFingerprintV1,
}

impl OdrRegistrationFingerprintV1 {
    pub const fn group(self) -> OdrGroupId {
        self.group
    }

    pub const fn member(self) -> OdrMemberId {
        self.member
    }

    pub const fn role(self) -> OdrMemberRole {
        OdrMemberRole::RegistrationRecord
    }

    pub const fn abi(self) -> OdrAbiFingerprintV1 {
        self.abi
    }

    pub const fn lir(self) -> Digest256 {
        self.lir
    }

    pub const fn definition(self) -> OdrDefinitionFingerprintV1 {
        self.definition
    }
}

pub(super) fn callable_registration(
    group: OdrGroupId,
    member: OdrMemberId,
    plan: scoop_lir::StrongCallableRegistrationPlanV1,
    object: ObjectDefinitionFingerprintV1,
) -> Result<OdrRegistrationFingerprintV1, HashError> {
    registration(
        group,
        member,
        RegistrationProjection::Callable(plan),
        plan.primary_atom(),
        plan.registration_object_node(),
        object,
        None,
    )
}

pub(super) fn safepoint_registration(
    group: OdrGroupId,
    member: OdrMemberId,
    plan: scoop_lir::StrongSafepointRegistrationPlanV1,
    object_node: DigestNodeId,
    object: ObjectDefinitionFingerprintV1,
    stackmap: StackmapRecordFingerprintV1,
) -> Result<OdrRegistrationFingerprintV1, HashError> {
    registration(
        group,
        member,
        RegistrationProjection::Safepoint(plan),
        plan.primary_atom(),
        object_node,
        object,
        Some((plan.site(), stackmap)),
    )
}

#[allow(clippy::too_many_arguments)]
fn registration(
    group: OdrGroupId,
    member: OdrMemberId,
    projection: RegistrationProjection,
    atom: ObjectDefinitionAtomId,
    object_node: DigestNodeId,
    object: ObjectDefinitionFingerprintV1,
    stackmap: Option<(PersistentSafepointSiteId, StackmapRecordFingerprintV1)>,
) -> Result<OdrRegistrationFingerprintV1, HashError> {
    let abi = domain_separated_cbor_hash(
        "scoop-odr-member-abi-v1",
        &AbiInput {
            group,
            member,
            projection,
        },
    )?;
    let lir = domain_separated_cbor_hash("scoop-lir-definition-v1", &projection)?;
    let definition = domain_separated_cbor_hash(
        "scoop-odr-member-definition-v1",
        &DefinitionInput {
            group,
            member,
            atom,
            lir,
            object_node,
            object,
            stackmap,
        },
    )?;
    Ok(OdrRegistrationFingerprintV1 {
        group,
        member,
        abi: OdrAbiFingerprintV1::from_array(*abi.as_array()),
        lir,
        definition: OdrDefinitionFingerprintV1::from_array(*definition.as_array()),
    })
}
