use std::fmt;

use scoop_identity::{
    CapabilityIdError, CapabilityRefinementError, DecodedCapabilityId, ObjectFormatId,
    TargetProfileWireId,
};
use scoop_wire::{Decoder, Digest256, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    ExtensionRequirement, LogicalMemberKey, LogicalMemberKeyError, MemberStableKey, SlibMemberId,
    SlibMemberRecord, SlibMemberRecordError, SlibMemberRole, WIRE_SCHEMA, validate_key_role,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedLogicalMemberKey(Vec<u8>);

impl DecodedLogicalMemberKey {
    fn validate(self) -> Result<LogicalMemberKey, LogicalMemberKeyError> {
        LogicalMemberKey::new(self.0)
    }
}

impl WireEncode for DecodedLogicalMemberKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl WireDecode for DecodedLogicalMemberKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.owned_bytes().map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedMemberStableKey {
    HirMetadata,
    MirMetadata,
    LirMetadata,
    LinkObject {
        verifier_capability: DecodedCapabilityId,
        logical_key: DecodedLogicalMemberKey,
    },
    DiagnosticAttachment {
        capability: DecodedCapabilityId,
        logical_key: DecodedLogicalMemberKey,
    },
    ExtensionBlob {
        capability: DecodedCapabilityId,
        logical_key: DecodedLogicalMemberKey,
    },
}

impl DecodedMemberStableKey {
    fn validate(self) -> Result<MemberStableKey, MemberStableKeyValidationError> {
        match self {
            Self::HirMetadata => Ok(MemberStableKey::HirMetadata),
            Self::MirMetadata => Ok(MemberStableKey::MirMetadata),
            Self::LirMetadata => Ok(MemberStableKey::LirMetadata),
            Self::LinkObject {
                verifier_capability,
                logical_key,
            } => Ok(MemberStableKey::LinkObject {
                verifier_capability: verifier_capability
                    .validate()
                    .map_err(MemberStableKeyValidationError::Capability)?,
                logical_key: logical_key
                    .validate()
                    .map_err(MemberStableKeyValidationError::LogicalKey)?,
            }),
            Self::DiagnosticAttachment {
                capability,
                logical_key,
            } => Ok(MemberStableKey::DiagnosticAttachment {
                capability: capability
                    .validate()
                    .map_err(MemberStableKeyValidationError::Capability)?,
                logical_key: logical_key
                    .validate()
                    .map_err(MemberStableKeyValidationError::LogicalKey)?,
            }),
            Self::ExtensionBlob {
                capability,
                logical_key,
            } => Ok(MemberStableKey::ExtensionBlob {
                capability: capability
                    .validate()
                    .map_err(MemberStableKeyValidationError::Capability)?,
                logical_key: logical_key
                    .validate()
                    .map_err(MemberStableKeyValidationError::LogicalKey)?,
            }),
        }
    }
}

impl WireEncode for DecodedMemberStableKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::HirMetadata => encode_empty_sum(encoder, 1),
            Self::MirMetadata => encode_empty_sum(encoder, 2),
            Self::LirMetadata => encode_empty_sum(encoder, 3),
            Self::LinkObject {
                verifier_capability,
                logical_key,
            } => encode_capability_key(encoder, 4, verifier_capability, logical_key),
            Self::DiagnosticAttachment {
                capability,
                logical_key,
            } => encode_capability_key(encoder, 5, capability, logical_key),
            Self::ExtensionBlob {
                capability,
                logical_key,
            } => encode_capability_key(encoder, 6, capability, logical_key),
        }
    }
}

impl WireDecode for DecodedMemberStableKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::HirMetadata)
            }
            2 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::MirMetadata)
            }
            3 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::LirMetadata)
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::LinkObject {
                    verifier_capability: decoder.field(1, DecodedCapabilityId::decode)?,
                    logical_key: decoder.field(2, DecodedLogicalMemberKey::decode)?,
                })
            }
            5 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::DiagnosticAttachment {
                    capability: decoder.field(1, DecodedCapabilityId::decode)?,
                    logical_key: decoder.field(2, DecodedLogicalMemberKey::decode)?,
                })
            }
            6 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ExtensionBlob {
                    capability: decoder.field(1, DecodedCapabilityId::decode)?,
                    logical_key: decoder.field(2, DecodedLogicalMemberKey::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedSlibMemberRole {
    HirMetadata {
        wire_schema: u32,
    },
    MirMetadata {
        wire_schema: u32,
    },
    LirMetadata {
        wire_schema: u32,
    },
    LinkObject {
        target_profile: DecodedCapabilityId,
        object_format: DecodedCapabilityId,
        verifier_capability: DecodedCapabilityId,
    },
    DiagnosticAttachment {
        capability: DecodedCapabilityId,
    },
    ExtensionBlob {
        capability: DecodedCapabilityId,
        required_for: u32,
    },
}

impl DecodedSlibMemberRole {
    fn validate(self) -> Result<SlibMemberRole, SlibMemberRoleValidationError> {
        match self {
            Self::HirMetadata { wire_schema } => {
                validate_wire_schema(wire_schema)?;
                Ok(SlibMemberRole::HirMetadata)
            }
            Self::MirMetadata { wire_schema } => {
                validate_wire_schema(wire_schema)?;
                Ok(SlibMemberRole::MirMetadata)
            }
            Self::LirMetadata { wire_schema } => {
                validate_wire_schema(wire_schema)?;
                Ok(SlibMemberRole::LirMetadata)
            }
            Self::LinkObject {
                target_profile,
                object_format,
                verifier_capability,
            } => {
                let target_profile = target_profile
                    .validate()
                    .map_err(SlibMemberRoleValidationError::Capability)
                    .and_then(|capability| {
                        TargetProfileWireId::refine(capability)
                            .map_err(SlibMemberRoleValidationError::TargetProfile)
                    })?;
                let object_format = object_format
                    .validate()
                    .map_err(SlibMemberRoleValidationError::Capability)
                    .and_then(|capability| {
                        ObjectFormatId::refine(capability)
                            .map_err(SlibMemberRoleValidationError::ObjectFormat)
                    })?;
                let verifier_capability = verifier_capability
                    .validate()
                    .map_err(SlibMemberRoleValidationError::Capability)?;
                Ok(SlibMemberRole::LinkObject {
                    target_profile,
                    object_format,
                    verifier_capability,
                })
            }
            Self::DiagnosticAttachment { capability } => Ok(SlibMemberRole::DiagnosticAttachment {
                capability: capability
                    .validate()
                    .map_err(SlibMemberRoleValidationError::Capability)?,
            }),
            Self::ExtensionBlob {
                capability,
                required_for,
            } => {
                let requirement = match required_for {
                    0 => ExtensionRequirement::Optional,
                    4 => ExtensionRequirement::Link,
                    bits => {
                        return Err(SlibMemberRoleValidationError::InvalidExtensionRequirement {
                            bits,
                        });
                    }
                };
                Ok(SlibMemberRole::ExtensionBlob {
                    capability: capability
                        .validate()
                        .map_err(SlibMemberRoleValidationError::Capability)?,
                    requirement,
                })
            }
        }
    }
}

impl WireEncode for DecodedSlibMemberRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::HirMetadata { wire_schema } => encode_schema_role(encoder, 1, *wire_schema),
            Self::MirMetadata { wire_schema } => encode_schema_role(encoder, 2, *wire_schema),
            Self::LirMetadata { wire_schema } => encode_schema_role(encoder, 3, *wire_schema),
            Self::LinkObject {
                target_profile,
                object_format,
                verifier_capability,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                target_profile.encode(encoder)?;
                encoder.field(2)?;
                object_format.encode(encoder)?;
                encoder.field(3)?;
                verifier_capability.encode(encoder)
            }
            Self::DiagnosticAttachment { capability } => {
                encoder.map(2)?;
                encode_tag(encoder, 5)?;
                encoder.field(1)?;
                capability.encode(encoder)
            }
            Self::ExtensionBlob {
                capability,
                required_for,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 6)?;
                encoder.field(1)?;
                capability.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(*required_for))
            }
        }
    }
}

impl WireDecode for DecodedSlibMemberRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::HirMetadata {
                    wire_schema: decoder.field(1, Decoder::u32)?,
                })
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::MirMetadata {
                    wire_schema: decoder.field(1, Decoder::u32)?,
                })
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::LirMetadata {
                    wire_schema: decoder.field(1, Decoder::u32)?,
                })
            }
            4 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::LinkObject {
                    target_profile: decoder.field(1, DecodedCapabilityId::decode)?,
                    object_format: decoder.field(2, DecodedCapabilityId::decode)?,
                    verifier_capability: decoder.field(3, DecodedCapabilityId::decode)?,
                })
            }
            5 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::DiagnosticAttachment {
                    capability: decoder.field(1, DecodedCapabilityId::decode)?,
                })
            }
            6 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ExtensionBlob {
                    capability: decoder.field(1, DecodedCapabilityId::decode)?,
                    required_for: decoder.field(2, Decoder::u32)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedSlibMemberId([u8; 32]);

impl WireEncode for DecodedSlibMemberId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl WireDecode for DecodedSlibMemberId {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let bytes = decoder.bytes()?;
        let fixed = <&[u8; 32]>::try_from(bytes).map_err(|_| {
            wire_error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected: 32,
                    actual: bytes.len() as u64,
                },
            )
        })?;
        Ok(Self(*fixed))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSlibMemberRecord {
    id: DecodedSlibMemberId,
    stable_key: DecodedMemberStableKey,
    role: DecodedSlibMemberRole,
    byte_length: u64,
    sha256: Digest256,
}

impl DecodedSlibMemberRecord {
    pub fn validate(
        self,
        cone: scoop_identity::ConeIdentity,
    ) -> Result<SlibMemberRecord, SlibMemberRecordValidationError> {
        let stable_key = self
            .stable_key
            .validate()
            .map_err(SlibMemberRecordValidationError::StableKey)?;
        let role = self
            .role
            .validate()
            .map_err(SlibMemberRecordValidationError::Role)?;
        validate_key_role(&stable_key, &role).map_err(SlibMemberRecordValidationError::KeyRole)?;
        let expected = SlibMemberId::from_stable_key(cone, &stable_key)
            .map_err(SlibMemberRecordValidationError::Hash)?;
        if self.id.0 != *expected.as_array() {
            return Err(SlibMemberRecordValidationError::IdMismatch {
                expected,
                actual: self.id.0,
            });
        }
        Ok(SlibMemberRecord {
            id: expected,
            stable_key,
            role,
            byte_length: self.byte_length,
            sha256: self.sha256,
        })
    }
}

impl WireEncode for DecodedSlibMemberRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        self.stable_key.encode(encoder)?;
        encoder.field(3)?;
        self.role.encode(encoder)?;
        encoder.field(4)?;
        encoder.unsigned(self.byte_length)?;
        encoder.field(5)?;
        self.sha256.encode(encoder)
    }
}

impl WireDecode for DecodedSlibMemberRecord {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            id: decoder.field(1, DecodedSlibMemberId::decode)?,
            stable_key: decoder.field(2, DecodedMemberStableKey::decode)?,
            role: decoder.field(3, DecodedSlibMemberRole::decode)?,
            byte_length: decoder.field(4, Decoder::unsigned)?,
            sha256: decoder.field(5, Digest256::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemberStableKeyValidationError {
    Capability(CapabilityIdError),
    LogicalKey(LogicalMemberKeyError),
}

impl fmt::Display for MemberStableKeyValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(error) => error.fmt(formatter),
            Self::LogicalKey(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for MemberStableKeyValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlibMemberRoleValidationError {
    UnsupportedWireSchema { actual: u32 },
    Capability(CapabilityIdError),
    TargetProfile(CapabilityRefinementError),
    ObjectFormat(CapabilityRefinementError),
    InvalidExtensionRequirement { bits: u32 },
}

impl fmt::Display for SlibMemberRoleValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedWireSchema { actual } => {
                write!(formatter, "metadata wire schema must be 1, found {actual}")
            }
            Self::Capability(error) => error.fmt(formatter),
            Self::TargetProfile(error) => write!(formatter, "invalid target profile: {error}"),
            Self::ObjectFormat(error) => write!(formatter, "invalid object format: {error}"),
            Self::InvalidExtensionRequirement { bits } => write!(
                formatter,
                "extension required_for must be 0 or Link (4), found {bits:#x}"
            ),
        }
    }
}

impl std::error::Error for SlibMemberRoleValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlibMemberRecordValidationError {
    StableKey(MemberStableKeyValidationError),
    Role(SlibMemberRoleValidationError),
    KeyRole(SlibMemberRecordError),
    Hash(scoop_wire::HashError),
    IdMismatch {
        expected: SlibMemberId,
        actual: [u8; 32],
    },
}

impl fmt::Display for SlibMemberRecordValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StableKey(error) => error.fmt(formatter),
            Self::Role(error) => error.fmt(formatter),
            Self::KeyRole(error) => error.fmt(formatter),
            Self::Hash(error) => error.fmt(formatter),
            Self::IdMismatch { expected, actual } => {
                write!(
                    formatter,
                    "member id does not match canonical key: expected {expected}, found "
                )?;
                for byte in actual {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for SlibMemberRecordValidationError {}

fn validate_wire_schema(wire_schema: u32) -> Result<(), SlibMemberRoleValidationError> {
    if u64::from(wire_schema) == WIRE_SCHEMA {
        Ok(())
    } else {
        Err(SlibMemberRoleValidationError::UnsupportedWireSchema {
            actual: wire_schema,
        })
    }
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_capability_key(
    encoder: &mut Encoder,
    tag: u64,
    capability: &DecodedCapabilityId,
    logical_key: &DecodedLogicalMemberKey,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    capability.encode(encoder)?;
    encoder.field(2)?;
    logical_key.encode(encoder)
}

fn encode_schema_role(
    encoder: &mut Encoder,
    tag: u64,
    wire_schema: u32,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    encoder.unsigned(u64::from(wire_schema))
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

#[cfg(test)]
mod tests;
