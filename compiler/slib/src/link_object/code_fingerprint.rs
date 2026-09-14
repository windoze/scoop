//! Canonical semantic inputs for the single-Cone code fingerprint.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    CapabilityId, ConeIdentity, NativeExternalContract, NativeExternalContractFingerprint,
    NativeExternalSymbolKey, PersistentNativeExternalSymbolId,
};
use scoop_lir::CanonicalNativeExternalRequirementSurfaceV1;
use scoop_wire::{Digest256, Encoder, HashError, WireEncode};

use super::VerifiedEntryPatchSetV1;
use crate::{
    LinkMemberFingerprint, MemberStableKey, SlibMemberId, SlibMemberRecord, SlibMemberRole,
};

/// One final LinkObject directory identity and its content-bound fingerprint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedCodeLinkObjectMemberV1 {
    member: SlibMemberId,
    fingerprint: LinkMemberFingerprint,
}

impl VerifiedCodeLinkObjectMemberV1 {
    pub const fn member(self) -> SlibMemberId {
        self.member
    }

    pub const fn fingerprint(self) -> LinkMemberFingerprint {
        self.fingerprint
    }
}

/// Finalized built-in object bytes bound one-to-one to their directory records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCodeLinkObjectMemberSetV1 {
    final_objects: VerifiedEntryPatchSetV1,
    members: Vec<VerifiedCodeLinkObjectMemberV1>,
}

impl VerifiedCodeLinkObjectMemberSetV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.final_objects.entry().patch_sites().producer()
    }

    pub const fn final_objects(&self) -> &VerifiedEntryPatchSetV1 {
        &self.final_objects
    }

    pub fn members(&self) -> &[VerifiedCodeLinkObjectMemberV1] {
        &self.members
    }
}

impl WireEncode for VerifiedCodeLinkObjectMemberSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.members.len() as u64)?;
        for member in &self.members {
            member.fingerprint.encode(encoder)?;
        }
        Ok(())
    }
}

/// Bind every built-in LinkObject directory record to the final verified bytes.
pub fn verify_code_link_object_members_v1(
    final_objects: VerifiedEntryPatchSetV1,
    directory: &[SlibMemberRecord],
) -> Result<VerifiedCodeLinkObjectMemberSetV1, CodeLinkObjectMemberValidationError> {
    let expected = expected_final_members(&final_objects)?;
    let members = verify_directory_records(&expected, directory)?;
    Ok(VerifiedCodeLinkObjectMemberSetV1 {
        final_objects,
        members,
    })
}

#[derive(Clone)]
struct ExpectedFinalLinkObjectMemberV1 {
    member: SlibMemberId,
    stable_key: MemberStableKey,
    role: SlibMemberRole,
    byte_length: u64,
    content_digest: Digest256,
}

fn expected_final_members(
    final_objects: &VerifiedEntryPatchSetV1,
) -> Result<Vec<ExpectedFinalLinkObjectMemberV1>, CodeLinkObjectMemberValidationError> {
    let builtins = final_objects.entry().patch_sites().builtins();
    let member_plan = builtins.member_plan();
    let scoop_objects = final_objects
        .objects()
        .iter()
        .map(|object| (object.member(), object))
        .collect::<BTreeMap<_, _>>();
    let generated_objects = builtins
        .c_bridge_production()
        .members()
        .iter()
        .map(|object| (object.plan().member_id(), object))
        .collect::<BTreeMap<_, _>>();
    let mut expected = Vec::with_capacity(
        member_plan.scoop_lir_members().len() + member_plan.generated_bridge_members().len(),
    );
    for member in member_plan.scoop_lir_members() {
        let object = scoop_objects.get(&member.member_id()).ok_or(
            CodeLinkObjectMemberValidationError::FinalProofMissingMember(member.member_id()),
        )?;
        let envelope = object.envelope().sections().envelope();
        expected.push(ExpectedFinalLinkObjectMemberV1 {
            member: member.member_id(),
            stable_key: member.stable_key().clone(),
            role: member.role().clone(),
            byte_length: envelope.byte_length(),
            content_digest: envelope.content_digest(),
        });
    }
    for member in member_plan.generated_bridge_members() {
        let object = generated_objects.get(&member.member_id()).ok_or(
            CodeLinkObjectMemberValidationError::FinalProofMissingMember(member.member_id()),
        )?;
        let envelope = object.envelope().sections().envelope();
        expected.push(ExpectedFinalLinkObjectMemberV1 {
            member: member.member_id(),
            stable_key: member.stable_key().clone(),
            role: member.role().clone(),
            byte_length: envelope.byte_length(),
            content_digest: envelope.content_digest(),
        });
    }
    expected.sort_unstable_by_key(|member| member.member);
    if let Some(pair) = expected
        .windows(2)
        .find(|pair| pair[0].member == pair[1].member)
    {
        return Err(CodeLinkObjectMemberValidationError::FinalProofDuplicateMember(pair[0].member));
    }
    Ok(expected)
}

fn verify_directory_records(
    expected: &[ExpectedFinalLinkObjectMemberV1],
    directory: &[SlibMemberRecord],
) -> Result<Vec<VerifiedCodeLinkObjectMemberV1>, CodeLinkObjectMemberValidationError> {
    let mut seen = BTreeSet::new();
    let mut actual = BTreeMap::new();
    for record in directory {
        if !seen.insert(record.id()) {
            return Err(CodeLinkObjectMemberValidationError::DuplicateDirectoryMember(record.id()));
        }
        match record.role() {
            SlibMemberRole::LinkObject { .. } => {
                actual.insert(record.id(), record);
            }
            SlibMemberRole::ExtensionBlob {
                capability,
                requirement: crate::ExtensionRequirement::Link,
            } => {
                return Err(
                    CodeLinkObjectMemberValidationError::UnsupportedLinkExtension(
                        capability.clone(),
                    ),
                );
            }
            SlibMemberRole::HirMetadata
            | SlibMemberRole::MirMetadata
            | SlibMemberRole::LirMetadata
            | SlibMemberRole::DiagnosticAttachment { .. }
            | SlibMemberRole::ExtensionBlob {
                requirement: crate::ExtensionRequirement::Optional,
                ..
            } => {}
        }
    }

    let expected_ids = expected
        .iter()
        .map(|member| member.member)
        .collect::<BTreeSet<_>>();
    let actual_ids = actual.keys().copied().collect::<BTreeSet<_>>();
    if let Some(member) = expected_ids.difference(&actual_ids).next() {
        return Err(CodeLinkObjectMemberValidationError::MissingDirectoryMember(
            *member,
        ));
    }
    if let Some(member) = actual_ids.difference(&expected_ids).next() {
        return Err(CodeLinkObjectMemberValidationError::UnexpectedDirectoryMember(*member));
    }

    let mut verified = Vec::with_capacity(expected.len());
    for expected in expected {
        let record = actual[&expected.member];
        if record.stable_key() != &expected.stable_key || record.role() != &expected.role {
            return Err(CodeLinkObjectMemberValidationError::MemberIdentityMismatch(
                expected.member,
            ));
        }
        if record.byte_length() != expected.byte_length
            || record.sha256() != expected.content_digest
        {
            return Err(CodeLinkObjectMemberValidationError::MemberContentMismatch(
                expected.member,
            ));
        }
        let fingerprint = record
            .as_link_member()
            .expect("a LinkObject directory record always has a link member view")
            .fingerprint()
            .map_err(CodeLinkObjectMemberValidationError::Hash)?;
        verified.push(VerifiedCodeLinkObjectMemberV1 {
            member: expected.member,
            fingerprint,
        });
    }
    Ok(verified)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodeLinkObjectMemberValidationError {
    FinalProofMissingMember(SlibMemberId),
    FinalProofDuplicateMember(SlibMemberId),
    DuplicateDirectoryMember(SlibMemberId),
    MissingDirectoryMember(SlibMemberId),
    UnexpectedDirectoryMember(SlibMemberId),
    UnsupportedLinkExtension(CapabilityId),
    MemberIdentityMismatch(SlibMemberId),
    MemberContentMismatch(SlibMemberId),
    Hash(HashError),
}

impl fmt::Display for CodeLinkObjectMemberValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid code link-object member set: {self:?}")
    }
}

impl std::error::Error for CodeLinkObjectMemberValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash(source) => Some(source),
            _ => None,
        }
    }
}

/// One native external contract stripped of source-only provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeExternalContractCodeRecordV1 {
    symbol_id: PersistentNativeExternalSymbolId,
    symbol_key: NativeExternalSymbolKey,
    fingerprint: NativeExternalContractFingerprint,
    contract: NativeExternalContract,
}

impl NativeExternalContractCodeRecordV1 {
    pub const fn symbol_id(&self) -> PersistentNativeExternalSymbolId {
        self.symbol_id
    }

    pub const fn symbol_key(&self) -> &NativeExternalSymbolKey {
        &self.symbol_key
    }

    pub const fn fingerprint(&self) -> NativeExternalContractFingerprint {
        self.fingerprint
    }

    pub const fn contract(&self) -> &NativeExternalContract {
        &self.contract
    }
}

impl WireEncode for NativeExternalContractCodeRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.symbol_id.encode(encoder)?;
        encoder.field(2)?;
        self.symbol_key.encode(encoder)?;
        encoder.field(3)?;
        self.fingerprint.encode(encoder)?;
        encoder.field(4)?;
        self.contract.encode(encoder)
    }
}

/// Canonical native contract contribution to `CodeFingerprint`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalNativeExternalContractCodeSetV1 {
    contracts: Vec<NativeExternalContractCodeRecordV1>,
}

impl CanonicalNativeExternalContractCodeSetV1 {
    pub fn from_requirement_surface(
        surface: &CanonicalNativeExternalRequirementSurfaceV1,
    ) -> Result<Self, NativeExternalContractCodeSetBuildError> {
        let mut contracts = surface
            .contracts()
            .iter()
            .map(|requirement| NativeExternalContractCodeRecordV1 {
                symbol_id: requirement.symbol_id(),
                symbol_key: requirement.symbol_key().clone(),
                fingerprint: requirement.fingerprint(),
                contract: requirement.contract().clone(),
            })
            .collect::<Vec<_>>();
        contracts.sort_unstable_by(|left, right| {
            left.symbol_key
                .native_link_symbol()
                .as_bytes()
                .cmp(right.symbol_key.native_link_symbol().as_bytes())
        });
        if let Some(pair) = contracts.windows(2).find(|pair| {
            pair[0].symbol_key.native_link_symbol().as_bytes()
                == pair[1].symbol_key.native_link_symbol().as_bytes()
        }) {
            return Err(NativeExternalContractCodeSetBuildError::DuplicateSymbol(
                pair[0].symbol_key.native_link_symbol().as_bytes().to_vec(),
            ));
        }
        Ok(Self { contracts })
    }

    pub fn contracts(&self) -> &[NativeExternalContractCodeRecordV1] {
        &self.contracts
    }
}

impl WireEncode for CanonicalNativeExternalContractCodeSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.contracts.len() as u64)?;
        for contract in &self.contracts {
            contract.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeExternalContractCodeSetBuildError {
    DuplicateSymbol(Vec<u8>),
}

impl fmt::Display for NativeExternalContractCodeSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid native external contract code set: {self:?}"
        )
    }
}

impl std::error::Error for NativeExternalContractCodeSetBuildError {}

#[cfg(test)]
mod tests;
