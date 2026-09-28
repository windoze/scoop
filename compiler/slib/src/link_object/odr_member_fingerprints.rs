//! Member-local fingerprints for callable bodies and runtime registrations.

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
use super::callable_registrations::object_definition::CanonicalDigestInputV1;
use encode::{AbiInput, DefinitionInput, RegistrationProjection};

/// The definition slot has one of two distinct digest owners and algorithms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationFingerprintV1 {
    Strong(StrongRegistrationFingerprintV1),
    Odr(OdrMemberFingerprintV1),
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

/// A callable's final definition follows its actual Strong or ODR ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableDefinitionFingerprintV1 {
    Strong(ObjectDefinitionFingerprintV1),
    Odr(OdrMemberFingerprintV1),
}

/// Content of one physical member, independent of producer and placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OdrMemberFingerprintV1 {
    group: OdrGroupId,
    member: OdrMemberId,
    role: OdrMemberRole,
    abi: OdrAbiFingerprintV1,
    lir: Digest256,
    definition: OdrDefinitionFingerprintV1,
}

impl OdrMemberFingerprintV1 {
    pub const fn group(self) -> OdrGroupId {
        self.group
    }

    pub const fn member(self) -> OdrMemberId {
        self.member
    }

    pub const fn role(self) -> OdrMemberRole {
        self.role
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

pub(super) fn callable_body(
    canonical: scoop_lir::CanonicalCallableLirDefinitionV1,
    plan: scoop_lir::StrongCallableRegistrationPlanV1,
    object: ObjectDefinitionFingerprintV1,
    stackmaps: &[(PersistentSafepointSiteId, StackmapRecordFingerprintV1)],
) -> Result<CallableDefinitionFingerprintV1, HashError> {
    use scoop_lir::CanonicalCallableDefinitionOwnerV1;
    match canonical.owner() {
        CanonicalCallableDefinitionOwnerV1::Strong => {
            Ok(CallableDefinitionFingerprintV1::Strong(object))
        }
        CanonicalCallableDefinitionOwnerV1::Odr {
            group,
            member,
            role,
            abi,
        } => member_fingerprint(
            abi,
            DefinitionInput {
                group,
                member,
                role,
                atom: plan.body_primary_atom(),
                lir: canonical.fingerprint(),
                object_node: plan.body_definition_node(),
                object,
                additional_inputs: &[],
                stackmaps,
            },
        )
        .map(CallableDefinitionFingerprintV1::Odr),
    }
}

pub(super) fn callable_registration(
    group: OdrGroupId,
    member: OdrMemberId,
    plan: scoop_lir::StrongCallableRegistrationPlanV1,
    object: ObjectDefinitionFingerprintV1,
) -> Result<OdrMemberFingerprintV1, HashError> {
    registration(
        group,
        member,
        RegistrationProjection::Callable(plan),
        plan.primary_atom(),
        plan.registration_object_node(),
        object,
        None,
        &[],
    )
}

pub(super) fn shape_definition(
    canonical: scoop_lir::CanonicalShapeLirDefinitionV1,
    object: ObjectDefinitionFingerprintV1,
) -> Result<OdrMemberFingerprintV1, HashError> {
    let object_node = DigestNodeId::from_key(&scoop_identity::DigestNodeKey::object_definition(
        canonical.primary_atom(),
    ))?;
    member_fingerprint(
        canonical.abi(),
        DefinitionInput {
            group: canonical.group(),
            member: canonical.member(),
            role: canonical.role(),
            atom: canonical.primary_atom(),
            lir: canonical.fingerprint(),
            object_node,
            object,
            additional_inputs: &[],
            stackmaps: &[],
        },
    )
}

pub(super) fn type_registration<D: Copy, C>(
    group: OdrGroupId,
    member: OdrMemberId,
    plan: &scoop_lir::StrongTypeRegistrationPlan<D, C>,
    object: ObjectDefinitionFingerprintV1,
) -> Result<OdrMemberFingerprintV1, HashError> {
    registration(
        group,
        member,
        RegistrationProjection::Type {
            exact: plan.exact_type(),
            runtime: plan.runtime_type(),
            descriptor: plan.descriptor_symbol(),
            layout: plan.layout(),
        },
        plan.primary_atom(),
        plan.registration_object_node(),
        object,
        None,
        &[],
    )
}

pub(super) fn safepoint_registration(
    group: OdrGroupId,
    member: OdrMemberId,
    plan: scoop_lir::StrongSafepointRegistrationPlanV1,
    object_node: DigestNodeId,
    object: ObjectDefinitionFingerprintV1,
    stackmap: StackmapRecordFingerprintV1,
) -> Result<OdrMemberFingerprintV1, HashError> {
    registration(
        group,
        member,
        RegistrationProjection::Safepoint(plan),
        plan.primary_atom(),
        object_node,
        object,
        Some((plan.site(), stackmap)),
        &[],
    )
}

pub(super) fn immortal_registration(
    group: OdrGroupId,
    member: OdrMemberId,
    plan: scoop_lir::StrongImmortalObjectRegistrationPlanV1,
    object: ObjectDefinitionFingerprintV1,
    immortal: ObjectDefinitionFingerprintV1,
) -> Result<OdrMemberFingerprintV1, HashError> {
    registration(
        group,
        member,
        RegistrationProjection::Immortal(plan),
        plan.registration_primary_atom(),
        plan.registration_object_node(),
        object,
        None,
        &[object_input(plan.object_definition_node(), immortal)],
    )
}

pub(super) fn static_storage_registration(
    group: OdrGroupId,
    member: OdrMemberId,
    plan: &scoop_lir::StrongStaticStorageRegistrationPlanV1,
    object: ObjectDefinitionFingerprintV1,
    storage: ObjectDefinitionFingerprintV1,
    layout: super::LayoutFingerprintV1,
    scan: super::ScanFingerprintV1,
) -> Result<OdrMemberFingerprintV1, HashError> {
    registration(
        group,
        member,
        RegistrationProjection::StaticStorage(plan),
        plan.registration_primary_atom(),
        plan.registration_object_node(),
        object,
        None,
        &[
            object_input(plan.storage_definition_node(), storage),
            CanonicalDigestInputV1 {
                kind: DigestKind::Layout,
                node: plan.layout_fingerprint_node(),
                digest: *layout.as_array(),
            },
            CanonicalDigestInputV1 {
                kind: DigestKind::Scan,
                node: plan.scan_fingerprint_node(),
                digest: *scan.as_array(),
            },
        ],
    )
}

pub(super) fn initialization_registration<D>(
    group: OdrGroupId,
    member: OdrMemberId,
    plan: &scoop_lir::StrongInitializationUnitRegistrationPlan<D>,
    object: ObjectDefinitionFingerprintV1,
    cell: ObjectDefinitionFingerprintV1,
    descriptor: ObjectDefinitionFingerprintV1,
) -> Result<OdrMemberFingerprintV1, HashError> {
    let semantic = plan.semantic();
    registration(
        group,
        member,
        RegistrationProjection::Initialization {
            unit: semantic.unit(),
            diagnostic_path: semantic.diagnostic_path(),
            storage: semantic.storage(),
            failure_root: semantic.failure_root(),
            initializer: semantic.initializer(),
            ensure: semantic.ensure(),
            schedule: semantic.schedule(),
        },
        plan.registration_primary_atom(),
        plan.registration_object_node(),
        object,
        None,
        &[
            object_input(plan.cell_definition_node(), cell),
            object_input(plan.descriptor_definition_node(), descriptor),
        ],
    )
}

fn object_input(
    node: DigestNodeId,
    value: ObjectDefinitionFingerprintV1,
) -> CanonicalDigestInputV1 {
    CanonicalDigestInputV1 {
        kind: DigestKind::ObjectDefinition,
        node,
        digest: *value.as_array(),
    }
}

#[allow(clippy::too_many_arguments)]
fn registration(
    group: OdrGroupId,
    member: OdrMemberId,
    projection: RegistrationProjection<'_>,
    atom: ObjectDefinitionAtomId,
    object_node: DigestNodeId,
    object: ObjectDefinitionFingerprintV1,
    stackmap: Option<(PersistentSafepointSiteId, StackmapRecordFingerprintV1)>,
    additional_inputs: &[CanonicalDigestInputV1],
) -> Result<OdrMemberFingerprintV1, HashError> {
    let abi = domain_separated_cbor_hash(
        "scoop-odr-member-abi-v1",
        &AbiInput {
            group,
            member,
            projection,
        },
    )?;
    let lir = domain_separated_cbor_hash("scoop-lir-definition-v1", &projection)?;
    member_fingerprint(
        abi,
        DefinitionInput {
            group,
            member,
            role: OdrMemberRole::RegistrationRecord,
            atom,
            lir,
            object_node,
            object,
            additional_inputs,
            stackmaps: stackmap.as_slice(),
        },
    )
}

fn member_fingerprint(
    abi: Digest256,
    input: DefinitionInput<'_>,
) -> Result<OdrMemberFingerprintV1, HashError> {
    let definition = domain_separated_cbor_hash("scoop-odr-member-definition-v1", &input)?;
    Ok(OdrMemberFingerprintV1 {
        group: input.group,
        member: input.member,
        role: input.role,
        abi: OdrAbiFingerprintV1::from_array(*abi.as_array()),
        lir: input.lir,
        definition: OdrDefinitionFingerprintV1::from_array(*definition.as_array()),
    })
}
