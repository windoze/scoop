//! Complete built-in member coverage through strong relocation resolution.

use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::ConeIdentity;

use super::{
    GeneratedCBridgeObjectCandidateV1, ObjectRelocationValidationError,
    PlannedGeneratedBridgeObjectMemberV1, PlannedLinkObjectMemberSetV1,
    PlannedMemberStrongObjectSymbolsV1, PlannedScoopLirObjectMemberV1,
    PlannedStrongObjectSymbolSetV1, ScoopLirObjectEnvelopeValidationError,
    StrongObjectDefinitionValidationError, StrongRelocationClosureValidationError,
    VerifiedCBridgeProductionEnvelopeSetV1, VerifiedCurrentConeStrongRelocationClosureV1,
    VerifiedMemberObjectRelocationIndexV1, validate_scoop_lir_llvm_22_1_object_envelope_v1,
    verify_current_cone_strong_relocation_closure_v1, verify_member_object_relocations_v1,
    verify_member_strong_object_definitions_v1,
};
use crate::SlibMemberId;

/// Untrusted LLVM-produced object bytes attributed to one planned Scoop LIR member.
#[derive(Clone, Copy, Debug)]
pub struct ScoopLirObjectCandidateV1<'bytes> {
    member: SlibMemberId,
    bytes: &'bytes [u8],
}

impl<'bytes> ScoopLirObjectCandidateV1<'bytes> {
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

/// Proof covering all built-in members through envelope, exact strong atom,
/// physical relocation, and current-Cone strong-target validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedBuiltinObjectStrongRelocationSetV1 {
    member_plan: PlannedLinkObjectMemberSetV1,
    c_bridge_production: VerifiedCBridgeProductionEnvelopeSetV1,
    strong_relocations: VerifiedCurrentConeStrongRelocationClosureV1,
}

impl VerifiedBuiltinObjectStrongRelocationSetV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.member_plan.producer()
    }

    pub const fn member_plan(&self) -> &PlannedLinkObjectMemberSetV1 {
        &self.member_plan
    }

    pub const fn c_bridge_production(&self) -> &VerifiedCBridgeProductionEnvelopeSetV1 {
        &self.c_bridge_production
    }

    pub const fn strong_relocations(&self) -> &VerifiedCurrentConeStrongRelocationClosureV1 {
        &self.strong_relocations
    }
}

pub fn verify_builtin_object_strong_relocations_v1(
    member_plan: &PlannedLinkObjectMemberSetV1,
    symbol_plan: &PlannedStrongObjectSymbolSetV1,
    scoop_objects: &[ScoopLirObjectCandidateV1<'_>],
    c_bridge_production: VerifiedCBridgeProductionEnvelopeSetV1,
    c_bridge_objects: &[GeneratedCBridgeObjectCandidateV1<'_>],
) -> Result<VerifiedBuiltinObjectStrongRelocationSetV1, BuiltinObjectSetValidationError> {
    validate_producers(member_plan, symbol_plan, &c_bridge_production)?;
    validate_symbol_plan_coverage(member_plan, symbol_plan)?;
    validate_scoop_candidate_coverage(member_plan, scoop_objects)?;
    validate_c_bridge_candidate_coverage(member_plan, &c_bridge_production, c_bridge_objects)?;

    let mut members = Vec::with_capacity(scoop_objects.len() + c_bridge_objects.len());
    for object in scoop_objects {
        let symbols = required_symbol_plan(symbol_plan, object.member)?;
        let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(object.bytes)
            .map_err(|source| BuiltinObjectSetValidationError::ScoopEnvelope {
                member: object.member,
                source,
            })?
            .into_sections();
        members.push(verify_member(
            object.member,
            object.bytes,
            sections,
            symbols,
        )?);
    }
    for (proof, object) in c_bridge_production.members().iter().zip(c_bridge_objects) {
        let member = object.member();
        if proof.plan().member_id() != member {
            return Err(
                BuiltinObjectSetValidationError::CBridgeObjectMemberMismatch {
                    expected: proof.plan().member_id(),
                    actual: member,
                },
            );
        }
        let symbols = required_symbol_plan(symbol_plan, member)?;
        members.push(verify_member(
            member,
            object.bytes(),
            proof.envelope().sections().clone(),
            symbols,
        )?);
    }

    let strong_relocations = verify_current_cone_strong_relocation_closure_v1(members)
        .map_err(BuiltinObjectSetValidationError::StrongRelocationClosure)?;
    Ok(VerifiedBuiltinObjectStrongRelocationSetV1 {
        member_plan: member_plan.clone(),
        c_bridge_production,
        strong_relocations,
    })
}

fn validate_producers(
    member_plan: &PlannedLinkObjectMemberSetV1,
    symbol_plan: &PlannedStrongObjectSymbolSetV1,
    c_bridge_production: &VerifiedCBridgeProductionEnvelopeSetV1,
) -> Result<(), BuiltinObjectSetValidationError> {
    let expected = member_plan.producer();
    if symbol_plan.producer() != expected {
        return Err(BuiltinObjectSetValidationError::SymbolProducerMismatch {
            expected,
            actual: symbol_plan.producer(),
        });
    }
    if c_bridge_production.producer() != expected {
        return Err(
            BuiltinObjectSetValidationError::CBridgeProductionProducerMismatch {
                expected,
                actual: c_bridge_production.producer(),
            },
        );
    }
    Ok(())
}

fn validate_symbol_plan_coverage(
    member_plan: &PlannedLinkObjectMemberSetV1,
    symbol_plan: &PlannedStrongObjectSymbolSetV1,
) -> Result<(), BuiltinObjectSetValidationError> {
    let expected = planned_member_ids(member_plan);
    let actual = symbol_plan
        .members()
        .iter()
        .map(PlannedMemberStrongObjectSymbolsV1::member)
        .collect::<Vec<_>>();
    validate_exact_member_ids(&expected, &actual, MemberSetKind::StrongSymbols)
}

fn validate_scoop_candidate_coverage(
    member_plan: &PlannedLinkObjectMemberSetV1,
    objects: &[ScoopLirObjectCandidateV1<'_>],
) -> Result<(), BuiltinObjectSetValidationError> {
    let expected = member_plan
        .scoop_lir_members()
        .iter()
        .map(PlannedScoopLirObjectMemberV1::member_id)
        .collect::<Vec<_>>();
    let actual = objects
        .iter()
        .map(|object| object.member)
        .collect::<Vec<_>>();
    validate_exact_member_ids(&expected, &actual, MemberSetKind::ScoopObjects)
}

fn validate_c_bridge_candidate_coverage(
    member_plan: &PlannedLinkObjectMemberSetV1,
    proof: &VerifiedCBridgeProductionEnvelopeSetV1,
    objects: &[GeneratedCBridgeObjectCandidateV1<'_>],
) -> Result<(), BuiltinObjectSetValidationError> {
    let expected_plans = member_plan.generated_bridge_members();
    if proof.members().len() != expected_plans.len()
        || proof
            .members()
            .iter()
            .zip(expected_plans)
            .any(|(actual, expected)| actual.plan() != expected)
    {
        return Err(BuiltinObjectSetValidationError::CBridgeMemberPlanMismatch);
    }
    let expected = expected_plans
        .iter()
        .map(PlannedGeneratedBridgeObjectMemberV1::member_id)
        .collect::<Vec<_>>();
    let actual = objects
        .iter()
        .map(|object| object.member())
        .collect::<Vec<_>>();
    validate_exact_member_ids(&expected, &actual, MemberSetKind::CBridgeObjects)
}

fn planned_member_ids(member_plan: &PlannedLinkObjectMemberSetV1) -> Vec<SlibMemberId> {
    let mut members = member_plan
        .scoop_lir_members()
        .iter()
        .map(PlannedScoopLirObjectMemberV1::member_id)
        .chain(
            member_plan
                .generated_bridge_members()
                .iter()
                .map(PlannedGeneratedBridgeObjectMemberV1::member_id),
        )
        .collect::<Vec<_>>();
    members.sort_unstable();
    members
}

fn validate_exact_member_ids(
    expected: &[SlibMemberId],
    actual: &[SlibMemberId],
    kind: MemberSetKind,
) -> Result<(), BuiltinObjectSetValidationError> {
    for (index, pair) in actual.windows(2).enumerate() {
        if pair[0] >= pair[1] {
            return Err(if pair[0] == pair[1] {
                BuiltinObjectSetValidationError::DuplicateMember {
                    kind,
                    member: pair[0],
                }
            } else {
                BuiltinObjectSetValidationError::NonCanonicalMemberOrder {
                    kind,
                    index: index + 1,
                }
            });
        }
    }
    let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
    let actual_set = actual.iter().copied().collect::<BTreeSet<_>>();
    if let Some(member) = actual_set.difference(&expected_set).next() {
        return Err(BuiltinObjectSetValidationError::UnexpectedMember {
            kind,
            member: *member,
        });
    }
    if let Some(member) = expected_set.difference(&actual_set).next() {
        return Err(BuiltinObjectSetValidationError::MissingMember {
            kind,
            member: *member,
        });
    }
    Ok(())
}

fn required_symbol_plan(
    symbol_plan: &PlannedStrongObjectSymbolSetV1,
    member: SlibMemberId,
) -> Result<&PlannedMemberStrongObjectSymbolsV1, BuiltinObjectSetValidationError> {
    symbol_plan
        .member(member)
        .ok_or(BuiltinObjectSetValidationError::MissingMember {
            kind: MemberSetKind::StrongSymbols,
            member,
        })
}

fn verify_member(
    member: SlibMemberId,
    bytes: &[u8],
    sections: super::ValidatedBuiltinObjectSectionInventoryV1,
    symbols: &PlannedMemberStrongObjectSymbolsV1,
) -> Result<VerifiedMemberObjectRelocationIndexV1, BuiltinObjectSetValidationError> {
    let definitions = verify_member_strong_object_definitions_v1(bytes, sections, symbols)
        .map_err(|source| BuiltinObjectSetValidationError::StrongDefinitions { member, source })?;
    verify_member_object_relocations_v1(definitions)
        .map_err(|source| BuiltinObjectSetValidationError::Relocations { member, source })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberSetKind {
    StrongSymbols,
    ScoopObjects,
    CBridgeObjects,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinObjectSetValidationError {
    SymbolProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    CBridgeProductionProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    CBridgeMemberPlanMismatch,
    DuplicateMember {
        kind: MemberSetKind,
        member: SlibMemberId,
    },
    NonCanonicalMemberOrder {
        kind: MemberSetKind,
        index: usize,
    },
    UnexpectedMember {
        kind: MemberSetKind,
        member: SlibMemberId,
    },
    MissingMember {
        kind: MemberSetKind,
        member: SlibMemberId,
    },
    CBridgeObjectMemberMismatch {
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    ScoopEnvelope {
        member: SlibMemberId,
        source: ScoopLirObjectEnvelopeValidationError,
    },
    StrongDefinitions {
        member: SlibMemberId,
        source: StrongObjectDefinitionValidationError,
    },
    Relocations {
        member: SlibMemberId,
        source: ObjectRelocationValidationError,
    },
    StrongRelocationClosure(StrongRelocationClosureValidationError),
}

impl fmt::Display for BuiltinObjectSetValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid built-in object member set: {self:?}")
    }
}

impl std::error::Error for BuiltinObjectSetValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ScoopEnvelope { source, .. } => Some(source),
            Self::StrongDefinitions { source, .. } => Some(source),
            Self::Relocations { source, .. } => Some(source),
            Self::StrongRelocationClosure(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
