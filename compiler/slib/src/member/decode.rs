use std::fmt;

use scoop_identity::{
    CapabilityIdError, DecodedCapabilityId, NormalizedNativeSourcePath, NormalizedSourcePathError,
};
use scoop_wire::{Decoder, Digest256, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{ExtensionRequirement, SlibMemberId, SlibMemberRecord, SlibMemberRole, WIRE_SCHEMA};

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
    LinkObject,
    ObjectDictionary,
    NativeObject {
        source: String,
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
            Self::LinkObject => Ok(SlibMemberRole::LinkObject),
            Self::ObjectDictionary => Ok(SlibMemberRole::ObjectDictionary),
            Self::NativeObject { source } => Ok(SlibMemberRole::NativeObject {
                source: NormalizedNativeSourcePath::new(&source)
                    .map_err(SlibMemberRoleValidationError::NativeSource)?,
            }),
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
            Self::LinkObject => encode_empty_sum(encoder, 4),
            Self::ObjectDictionary => encode_empty_sum(encoder, 8),
            Self::NativeObject { source } => {
                encoder.map(2)?;
                encode_tag(encoder, 7)?;
                encoder.field(1)?;
                encoder.text(source)
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::LinkObject)
            }
            7 => {
                expect_sum_length(decoder, fields, 2)?;
                Ok(Self::NativeObject {
                    source: decoder.field(1, Decoder::owned_text)?,
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
            8 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::ObjectDictionary)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireDecode for SlibMemberId {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
    id: SlibMemberId,
    role: DecodedSlibMemberRole,
    byte_length: u64,
    sha256: Digest256,
}

impl DecodedSlibMemberRecord {
    pub fn validate(self) -> Result<SlibMemberRecord, SlibMemberRoleValidationError> {
        Ok(SlibMemberRecord {
            content: super::SlibMemberContent {
                id: self.id,
                role: self.role.validate()?,
                sha256: self.sha256,
            },
            byte_length: self.byte_length,
        })
    }
}

impl WireEncode for DecodedSlibMemberRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(3)?;
        self.role.encode(encoder)?;
        encoder.field(4)?;
        encoder.unsigned(self.byte_length)?;
        encoder.field(5)?;
        self.sha256.encode(encoder)
    }
}

impl WireDecode for DecodedSlibMemberRecord {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            id: decoder.field(1, SlibMemberId::decode)?,
            role: decoder.field(3, DecodedSlibMemberRole::decode)?,
            byte_length: decoder.field(4, Decoder::unsigned)?,
            sha256: decoder.field(5, Digest256::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlibMemberRoleValidationError {
    UnsupportedWireSchema { actual: u32 },
    Capability(CapabilityIdError),
    NativeSource(NormalizedSourcePathError),
    InvalidExtensionRequirement { bits: u32 },
}

impl fmt::Display for SlibMemberRoleValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedWireSchema { actual } => {
                write!(formatter, "metadata wire schema must be 1, found {actual}")
            }
            Self::Capability(error) => error.fmt(formatter),
            Self::NativeSource(error) => write!(formatter, "invalid native source path: {error}"),
            Self::InvalidExtensionRequirement { bits } => write!(
                formatter,
                "extension required_for must be 0 or Link (4), found {bits:#x}"
            ),
        }
    }
}

impl std::error::Error for SlibMemberRoleValidationError {}

fn validate_wire_schema(wire_schema: u32) -> Result<(), SlibMemberRoleValidationError> {
    if u64::from(wire_schema) == WIRE_SCHEMA {
        Ok(())
    } else {
        Err(SlibMemberRoleValidationError::UnsupportedWireSchema {
            actual: wire_schema,
        })
    }
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
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
