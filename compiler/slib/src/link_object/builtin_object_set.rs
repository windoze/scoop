//! Complete built-in member coverage through strong relocation resolution.

use std::fmt;

use scoop_identity::ConeIdentity;

use super::{
    GeneratedCBridgeObjectCandidateV1, GeneratedCObjectSetV1, ObjectRelocationValidationError,
    PlannedLinkObjectMemberSetV1, PlannedMemberStrongObjectSymbolsV1,
    PlannedStrongObjectSymbolSetV1, ScoopLirObjectEnvelopeValidationError,
    StrongObjectDefinitionValidationError, StrongRelocationClosureValidationError,
    VerifiedCurrentConeStrongRelocationClosureV1, VerifiedMemberObjectRelocationIndexV1,
    validate_scoop_lir_llvm_22_1_object_envelope_v1,
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

/// Member allocation and parsed relocation data used during object finalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedBuiltinObjectStrongRelocationSetV1 {
    member_plan: PlannedLinkObjectMemberSetV1,
    c_bridge_production: GeneratedCObjectSetV1,
    strong_relocations: VerifiedCurrentConeStrongRelocationClosureV1,
}

impl VerifiedBuiltinObjectStrongRelocationSetV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.member_plan.producer()
    }

    pub const fn member_plan(&self) -> &PlannedLinkObjectMemberSetV1 {
        &self.member_plan
    }

    pub const fn c_bridge_production(&self) -> &GeneratedCObjectSetV1 {
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
    c_bridge_production: GeneratedCObjectSetV1,
    c_bridge_objects: &[GeneratedCBridgeObjectCandidateV1<'_>],
) -> Result<VerifiedBuiltinObjectStrongRelocationSetV1, BuiltinObjectSetValidationError> {
    let mut members = Vec::with_capacity(scoop_objects.len() + c_bridge_objects.len());
    for object in scoop_objects {
        let symbols = required_symbol_plan(symbol_plan, object.member)?;
        let sections =
            validate_scoop_lir_llvm_22_1_object_envelope_v1(member_plan.target(), object.bytes)
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
        let symbols = required_symbol_plan(symbol_plan, member)?;
        members.push(verify_member(
            member,
            object.bytes(),
            proof.sections().clone(),
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

fn required_symbol_plan(
    symbol_plan: &PlannedStrongObjectSymbolSetV1,
    member: SlibMemberId,
) -> Result<&PlannedMemberStrongObjectSymbolsV1, BuiltinObjectSetValidationError> {
    symbol_plan
        .member(member)
        .ok_or(BuiltinObjectSetValidationError::MissingSymbolPlan(member))
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinObjectSetValidationError {
    MissingSymbolPlan(SlibMemberId),
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
