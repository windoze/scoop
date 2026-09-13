//! Request, plan, member, and Mach-O envelope binding for generated C objects.

use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{ConeIdentity, GeneratedBridgeUnitId};
use scoop_lir::{
    CBridgeProductionSetV1, CBridgeProductionV1, CBridgeToolchainProfileV1,
    GeneratedBridgePlanSetV1,
};

use super::{
    GeneratedCBridgeObjectEnvelopeValidationError, PlannedGeneratedBridgeObjectMemberV1,
    PlannedLinkObjectMemberSetV1, ValidatedGeneratedCBridgeObjectEnvelopeV1,
    validate_generated_c_bridge_object_envelope_v1,
};
use crate::SlibMemberId;

/// Untrusted object bytes attributed to one already planned generated-C member.
#[derive(Clone, Copy, Debug)]
pub struct GeneratedCBridgeObjectCandidateV1<'bytes> {
    member: SlibMemberId,
    bytes: &'bytes [u8],
}

impl<'bytes> GeneratedCBridgeObjectCandidateV1<'bytes> {
    pub const fn new(member: SlibMemberId, bytes: &'bytes [u8]) -> Self {
        Self { member, bytes }
    }

    pub const fn member(self) -> SlibMemberId {
        self.member
    }

    pub const fn bytes(self) -> &'bytes [u8] {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedGeneratedCBridgeMemberEnvelopeV1 {
    plan: PlannedGeneratedBridgeObjectMemberV1,
    envelope: ValidatedGeneratedCBridgeObjectEnvelopeV1,
}

impl VerifiedGeneratedCBridgeMemberEnvelopeV1 {
    pub const fn plan(&self) -> &PlannedGeneratedBridgeObjectMemberV1 {
        &self.plan
    }

    pub const fn envelope(&self) -> &ValidatedGeneratedCBridgeObjectEnvelopeV1 {
        &self.envelope
    }
}

/// Proof that the request profile and production manifest describe exactly the
/// generated-C object envelopes assigned by the member plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCBridgeProductionEnvelopeSetV1 {
    producer: ConeIdentity,
    production: CBridgeProductionSetV1,
    members: Vec<VerifiedGeneratedCBridgeMemberEnvelopeV1>,
}

impl VerifiedCBridgeProductionEnvelopeSetV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn production(&self) -> &CBridgeProductionSetV1 {
        &self.production
    }

    pub fn members(&self) -> &[VerifiedGeneratedCBridgeMemberEnvelopeV1] {
        &self.members
    }
}

pub fn verify_c_bridge_production_envelopes_v1(
    bridge_plan: &GeneratedBridgePlanSetV1,
    production: &CBridgeProductionSetV1,
    profile: &CBridgeToolchainProfileV1,
    member_plan: &PlannedLinkObjectMemberSetV1,
    objects: &[GeneratedCBridgeObjectCandidateV1<'_>],
) -> Result<VerifiedCBridgeProductionEnvelopeSetV1, CBridgeProductionEnvelopeValidationError> {
    if bridge_plan.producer() != member_plan.producer() {
        return Err(CBridgeProductionEnvelopeValidationError::ProducerMismatch {
            bridge_plan: bridge_plan.producer(),
            member_plan: member_plan.producer(),
        });
    }

    let expected_units = bridge_plan
        .units()
        .iter()
        .map(|unit| unit.unit())
        .collect::<Vec<_>>();
    validate_production(production, profile, &expected_units)?;
    validate_planned_unit_coverage(member_plan, &expected_units)?;
    validate_canonical_object_order(objects)?;

    let expected_members = member_plan
        .generated_bridge_members()
        .iter()
        .map(PlannedGeneratedBridgeObjectMemberV1::member_id)
        .collect::<BTreeSet<_>>();
    let actual_members = objects
        .iter()
        .map(|object| object.member)
        .collect::<BTreeSet<_>>();
    if let Some(member) = actual_members.difference(&expected_members).next() {
        return Err(CBridgeProductionEnvelopeValidationError::UnexpectedObjectMember(*member));
    }
    if let Some(member) = expected_members.difference(&actual_members).next() {
        return Err(CBridgeProductionEnvelopeValidationError::MissingObjectMember(*member));
    }

    let mut members = Vec::with_capacity(objects.len());
    for object in objects {
        let plan_index = member_plan
            .generated_bridge_members()
            .binary_search_by_key(
                &object.member,
                PlannedGeneratedBridgeObjectMemberV1::member_id,
            )
            .map_err(|_| {
                CBridgeProductionEnvelopeValidationError::UnexpectedObjectMember(object.member)
            })?;
        let plan = &member_plan.generated_bridge_members()[plan_index];
        let envelope = validate_generated_c_bridge_object_envelope_v1(
            object.bytes,
            profile.contract().deployment(),
        )
        .map_err(
            |source| CBridgeProductionEnvelopeValidationError::ObjectEnvelope {
                member: object.member,
                source,
            },
        )?;
        members.push(VerifiedGeneratedCBridgeMemberEnvelopeV1 {
            plan: plan.clone(),
            envelope,
        });
    }

    Ok(VerifiedCBridgeProductionEnvelopeSetV1 {
        producer: bridge_plan.producer(),
        production: production.clone(),
        members,
    })
}

fn validate_production(
    production: &CBridgeProductionSetV1,
    profile: &CBridgeToolchainProfileV1,
    expected_units: &[GeneratedBridgeUnitId],
) -> Result<(), CBridgeProductionEnvelopeValidationError> {
    match (expected_units.is_empty(), production) {
        (true, CBridgeProductionSetV1::NotUsed) => Ok(()),
        (true, CBridgeProductionSetV1::Used(_)) => {
            Err(CBridgeProductionEnvelopeValidationError::UnexpectedProduction)
        }
        (false, CBridgeProductionSetV1::NotUsed) => {
            Err(CBridgeProductionEnvelopeValidationError::MissingProduction)
        }
        (false, CBridgeProductionSetV1::Used(actual)) => {
            validate_used_production(actual, profile, expected_units)
        }
    }
}

fn validate_used_production(
    actual: &CBridgeProductionV1,
    profile: &CBridgeToolchainProfileV1,
    expected_units: &[GeneratedBridgeUnitId],
) -> Result<(), CBridgeProductionEnvelopeValidationError> {
    if actual.profile_id() != profile.id() {
        return Err(CBridgeProductionEnvelopeValidationError::ProfileIdMismatch);
    }
    if actual.profile_fingerprint() != profile.fingerprint() {
        return Err(CBridgeProductionEnvelopeValidationError::ProfileFingerprintMismatch);
    }
    if actual.source_template_fingerprint() != profile.contract().source_template_fingerprint() {
        return Err(CBridgeProductionEnvelopeValidationError::SourceTemplateMismatch);
    }
    if actual.canonical_flag_fingerprint() != profile.contract().canonical_flag_fingerprint() {
        return Err(CBridgeProductionEnvelopeValidationError::CanonicalFlagsMismatch);
    }
    if actual.units() != expected_units {
        return Err(CBridgeProductionEnvelopeValidationError::ProductionUnitCoverageMismatch);
    }
    Ok(())
}

fn validate_planned_unit_coverage(
    member_plan: &PlannedLinkObjectMemberSetV1,
    expected_units: &[GeneratedBridgeUnitId],
) -> Result<(), CBridgeProductionEnvelopeValidationError> {
    let expected = expected_units.iter().copied().collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    for member in member_plan.generated_bridge_members() {
        for unit in member.units().units() {
            if !actual.insert(*unit) {
                return Err(CBridgeProductionEnvelopeValidationError::DuplicatePlannedUnit(*unit));
            }
        }
    }
    if let Some(unit) = actual.difference(&expected).next() {
        return Err(CBridgeProductionEnvelopeValidationError::UnexpectedPlannedUnit(*unit));
    }
    if let Some(unit) = expected.difference(&actual).next() {
        return Err(CBridgeProductionEnvelopeValidationError::MissingPlannedUnit(*unit));
    }
    Ok(())
}

fn validate_canonical_object_order(
    objects: &[GeneratedCBridgeObjectCandidateV1<'_>],
) -> Result<(), CBridgeProductionEnvelopeValidationError> {
    for (index, pair) in objects.windows(2).enumerate() {
        if pair[0].member >= pair[1].member {
            return Err(if pair[0].member == pair[1].member {
                CBridgeProductionEnvelopeValidationError::DuplicateObjectMember(pair[0].member)
            } else {
                CBridgeProductionEnvelopeValidationError::NonCanonicalObjectOrder {
                    index: index + 1,
                }
            });
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CBridgeProductionEnvelopeValidationError {
    ProducerMismatch {
        bridge_plan: ConeIdentity,
        member_plan: ConeIdentity,
    },
    UnexpectedProduction,
    MissingProduction,
    ProfileIdMismatch,
    ProfileFingerprintMismatch,
    SourceTemplateMismatch,
    CanonicalFlagsMismatch,
    ProductionUnitCoverageMismatch,
    DuplicatePlannedUnit(GeneratedBridgeUnitId),
    UnexpectedPlannedUnit(GeneratedBridgeUnitId),
    MissingPlannedUnit(GeneratedBridgeUnitId),
    DuplicateObjectMember(SlibMemberId),
    NonCanonicalObjectOrder {
        index: usize,
    },
    UnexpectedObjectMember(SlibMemberId),
    MissingObjectMember(SlibMemberId),
    ObjectEnvelope {
        member: SlibMemberId,
        source: GeneratedCBridgeObjectEnvelopeValidationError,
    },
}

impl fmt::Display for CBridgeProductionEnvelopeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid generated-C production envelope set: {self:?}"
        )
    }
}

impl std::error::Error for CBridgeProductionEnvelopeValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ObjectEnvelope { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
