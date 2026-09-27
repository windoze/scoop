//! Decode directory structure and compare it with existing member contents.

use std::collections::BTreeSet;

use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, WireDecode, WireError, WireErrorKind};

use super::*;
use crate::link_object::DecodedFixedBytesV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedOdrMemberEntry {
    member: DecodedPersistentId<OdrMemberId>,
    role: OdrMemberRole,
    abi: DecodedFixedBytesV1<OdrAbiFingerprintV1>,
    definition: DecodedFixedBytesV1<OdrDefinitionFingerprintV1>,
}

impl WireDecode for DecodedOdrMemberEntry {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            member: decoder.field(1, DecodedPersistentId::decode)?,
            role: decoder.field(2, |decoder| {
                let role = OdrMemberRole::decode(decoder)?;
                if !physical_role(role) {
                    return Err(error(decoder, WireErrorKind::NonCanonicalCbor));
                }
                Ok(role)
            })?,
            abi: decoder.field(3, DecodedFixedBytesV1::decode)?,
            definition: decoder.field(4, DecodedFixedBytesV1::decode)?,
        })
    }
}

impl WireEncode for DecodedOdrMemberEntry {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.abi.encode(encoder)?;
        encoder.field(4)?;
        self.definition.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedOdrMemberGroup {
    group: DecodedPersistentId<OdrGroupId>,
    members: Vec<DecodedOdrMemberEntry>,
}

impl WireDecode for DecodedOdrMemberGroup {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let group = decoder.field(1, DecodedPersistentId::decode)?;
        let members = decoder.field(2, |decoder| {
            let mut previous = None;
            let members = decoder.decode_array(|decoder, _| {
                let entry = DecodedOdrMemberEntry::decode(decoder)?;
                if previous.is_some_and(|id| id >= entry.member) {
                    return Err(error(decoder, WireErrorKind::NonCanonicalCbor));
                }
                previous = Some(entry.member);
                Ok(entry)
            })?;
            if members.is_empty() {
                return Err(error(
                    decoder,
                    WireErrorKind::InvalidLength {
                        expected: 1,
                        actual: 0,
                    },
                ));
            }
            Ok(members)
        })?;
        Ok(Self { group, members })
    }
}

impl WireEncode for DecodedOdrMemberGroup {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.group.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.members)
    }
}

/// Parsed IDs and digests remain separate from the recomputed directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalOdrMemberDirectoryV1 {
    groups: Vec<DecodedOdrMemberGroup>,
}

impl WireDecode for DecodedCanonicalOdrMemberDirectoryV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let mut previous = None;
        let mut members = BTreeSet::new();
        let groups = decoder.decode_array(|decoder, _| {
            let group = DecodedOdrMemberGroup::decode(decoder)?;
            if previous.is_some_and(|id| id >= group.group)
                || group
                    .members
                    .iter()
                    .any(|entry| !members.insert(entry.member))
            {
                return Err(error(decoder, WireErrorKind::NonCanonicalCbor));
            }
            previous = Some(group.group);
            Ok(group)
        })?;
        Ok(Self { groups })
    }
}

impl WireEncode for DecodedCanonicalOdrMemberDirectoryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.groups)
    }
}

impl DecodedCanonicalOdrMemberDirectoryV1 {
    pub fn validate_against(
        &self,
        expected: &CanonicalOdrMemberDirectoryV1,
    ) -> Result<(), OdrMemberDirectoryValidationError> {
        if self.groups.len() != expected.groups.len() {
            return Err(OdrMemberDirectoryValidationError::GroupSetMismatch);
        }
        for (actual, expected) in self.groups.iter().zip(&expected.groups) {
            if actual.group.as_array() != expected.group.as_array() {
                return Err(OdrMemberDirectoryValidationError::GroupSetMismatch);
            }
            if actual.members.len() != expected.members.len() {
                return Err(OdrMemberDirectoryValidationError::MemberSetMismatch(
                    expected.group,
                ));
            }
            let group = expected.group;
            for (actual, expected) in actual.members.iter().zip(&expected.members) {
                if actual.member.as_array() != expected.member.as_array() {
                    return Err(OdrMemberDirectoryValidationError::MemberSetMismatch(group));
                }
                if actual.role != expected.role {
                    return Err(OdrMemberDirectoryValidationError::RoleMismatch(
                        expected.member,
                    ));
                }
                if !actual.abi.matches(expected.abi.as_array()) {
                    return Err(OdrMemberDirectoryValidationError::AbiMismatch(
                        expected.member,
                    ));
                }
                if !actual.definition.matches(expected.definition.as_array()) {
                    return Err(OdrMemberDirectoryValidationError::DefinitionMismatch(
                        expected.member,
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OdrMemberDirectoryValidationError {
    GroupSetMismatch,
    MemberSetMismatch(OdrGroupId),
    RoleMismatch(OdrMemberId),
    AbiMismatch(OdrMemberId),
    DefinitionMismatch(OdrMemberId),
}

impl fmt::Display for OdrMemberDirectoryValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ODR member directory differs from emitted definitions: {self:?}"
        )
    }
}

impl std::error::Error for OdrMemberDirectoryValidationError {}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
