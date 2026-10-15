//! Decode saved ODR member identities and ABI entries.

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
}

impl WireDecode for DecodedOdrMemberEntry {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
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
        })
    }
}

impl WireEncode for DecodedOdrMemberEntry {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.abi.encode(encoder)
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
    pub(crate) fn read(
        &self,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<CanonicalOdrMemberDirectoryV1, scoop_identity::IdentityReferenceError> {
        use scoop_identity::PersistentIdResolver;
        let groups = self
            .groups
            .iter()
            .map(|group| {
                Ok(OdrMemberDirectoryGroupV1 {
                    group: PersistentIdResolver::resolve(identities, group.group)?,
                    members: group
                        .members
                        .iter()
                        .map(|entry| {
                            Ok(OdrMemberDirectoryEntryV1 {
                                member: PersistentIdResolver::resolve(identities, entry.member)?,
                                role: entry.role,
                                abi: OdrAbiFingerprintV1::from_array(*entry.abi.bytes()),
                            })
                        })
                        .collect::<Result<_, scoop_identity::IdentityReferenceError>>()?,
                })
            })
            .collect::<Result<_, scoop_identity::IdentityReferenceError>>()?;
        Ok(CanonicalOdrMemberDirectoryV1 { groups })
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
