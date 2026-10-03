use std::fmt;

use scoop_identity::{ConeIdentity, ObjectFormatId, TargetProfileWireId};
use scoop_wire::{
    CanonicalHashStream, Digest256, Encoder, HashError, WireEncode, domain_separated_cbor_hash,
    sha256,
};

const MEMBER_ID_DOMAIN: &str = "scoop-slib-member-v1";
const MEMBER_FINGERPRINT_DOMAIN: &str = "scoop-slib-member-content-v1";
const LINK_MEMBER_FINGERPRINT_DOMAIN: &str = "scoop-slib-link-member-v1";
const WIRE_SCHEMA: u64 = 1;

macro_rules! typed_digest {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn as_array(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.bytes(&self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
        }
    };
}

typed_digest!(SlibMemberId);
typed_digest!(MemberFingerprint);
typed_digest!(LinkMemberFingerprint);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogicalMemberKey(Vec<u8>);

impl LogicalMemberKey {
    pub fn new(bytes: Vec<u8>) -> Result<Self, LogicalMemberKeyError> {
        match bytes.len() {
            0 => Err(LogicalMemberKeyError::Empty),
            _ => Ok(Self(bytes)),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalMemberKeyError {
    Empty,
}

impl fmt::Display for LogicalMemberKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("member logical key must not be empty"),
        }
    }
}

impl std::error::Error for LogicalMemberKeyError {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemberStableKey {
    HirMetadata,
    MirMetadata,
    LirMetadata,
    LinkObject {
        verifier_capability: scoop_identity::CapabilityId,
        logical_key: LogicalMemberKey,
    },
    DiagnosticAttachment {
        capability: scoop_identity::CapabilityId,
        logical_key: LogicalMemberKey,
    },
    ExtensionBlob {
        capability: scoop_identity::CapabilityId,
        logical_key: LogicalMemberKey,
    },
}

impl WireEncode for MemberStableKey {
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemberPurposeSet(u32);

impl MemberPurposeSet {
    pub const NONE: Self = Self(0);
    pub const GRAPH: Self = Self(0x0000_0001);
    pub const COMPILE: Self = Self(0x0000_0002);
    pub const LINK: Self = Self(0x0000_0004);
    pub const COMPILE_AND_LINK: Self = Self(Self::COMPILE.0 | Self::LINK.0);
    pub const DIAGNOSTICS: Self = Self(0x0000_0008);

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, purpose: Self) -> bool {
        self.0 & purpose.0 == purpose.0
    }

    pub(crate) const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }
}

impl WireEncode for MemberPurposeSet {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.0))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExtensionRequirement {
    Optional,
    Link,
}

impl ExtensionRequirement {
    pub const fn purpose_set(self) -> MemberPurposeSet {
        match self {
            Self::Optional => MemberPurposeSet::NONE,
            Self::Link => MemberPurposeSet::LINK,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlibMemberRole {
    HirMetadata,
    MirMetadata,
    LirMetadata,
    LinkObject {
        target_profile: TargetProfileWireId,
        object_format: ObjectFormatId,
        verifier_capability: scoop_identity::CapabilityId,
    },
    DiagnosticAttachment {
        capability: scoop_identity::CapabilityId,
    },
    ExtensionBlob {
        capability: scoop_identity::CapabilityId,
        requirement: ExtensionRequirement,
    },
}

impl SlibMemberRole {
    pub const fn purpose_set(&self) -> MemberPurposeSet {
        match self {
            Self::HirMetadata | Self::MirMetadata => MemberPurposeSet::COMPILE,
            Self::LirMetadata => {
                MemberPurposeSet(MemberPurposeSet::COMPILE.bits() | MemberPurposeSet::LINK.bits())
            }
            Self::LinkObject { .. } => MemberPurposeSet::LINK,
            Self::DiagnosticAttachment { .. } => MemberPurposeSet::DIAGNOSTICS,
            Self::ExtensionBlob { requirement, .. } => requirement.purpose_set(),
        }
    }
}

impl WireEncode for SlibMemberRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::HirMetadata => encode_schema_role(encoder, 1),
            Self::MirMetadata => encode_schema_role(encoder, 2),
            Self::LirMetadata => encode_schema_role(encoder, 3),
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
                requirement,
            } => {
                encoder.map(3)?;
                encode_tag(encoder, 6)?;
                encoder.field(1)?;
                capability.encode(encoder)?;
                encoder.field(2)?;
                requirement.purpose_set().encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlibMemberRecord {
    id: SlibMemberId,
    stable_key: MemberStableKey,
    role: SlibMemberRole,
    byte_length: u64,
    sha256: Digest256,
}

/// A directory record bound to the exact payload from which its length and
/// content digest were computed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlibMember {
    record: SlibMemberRecord,
    payload: Vec<u8>,
}

impl SlibMember {
    pub fn new(
        cone: ConeIdentity,
        stable_key: MemberStableKey,
        role: SlibMemberRole,
        payload: Vec<u8>,
    ) -> Result<Self, SlibMemberRecordError> {
        let record = SlibMemberRecord::new(cone, stable_key, role, &payload)?;
        Ok(Self { record, payload })
    }

    pub const fn record(&self) -> &SlibMemberRecord {
        &self.record
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl SlibMemberRecord {
    pub fn new(
        cone: ConeIdentity,
        stable_key: MemberStableKey,
        role: SlibMemberRole,
        payload: &[u8],
    ) -> Result<Self, SlibMemberRecordError> {
        validate_key_role(&stable_key, &role)?;
        let byte_length =
            u64::try_from(payload.len()).map_err(|_| SlibMemberRecordError::LengthOverflow)?;
        let id = SlibMemberId::from_stable_key(cone, &stable_key)
            .map_err(SlibMemberRecordError::Hash)?;
        Ok(Self {
            id,
            stable_key,
            role,
            byte_length,
            sha256: sha256(payload),
        })
    }

    pub const fn id(&self) -> SlibMemberId {
        self.id
    }

    pub const fn stable_key(&self) -> &MemberStableKey {
        &self.stable_key
    }

    pub const fn role(&self) -> &SlibMemberRole {
        &self.role
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn sha256(&self) -> Digest256 {
        self.sha256
    }

    pub fn fingerprint(&self) -> Result<MemberFingerprint, HashError> {
        domain_separated_cbor_hash(MEMBER_FINGERPRINT_DOMAIN, self)
            .map(|digest| MemberFingerprint(*digest.as_array()))
    }

    pub fn as_link_member(&self) -> Option<LinkMemberRecord<'_>> {
        match self.role {
            SlibMemberRole::LinkObject { .. }
            | SlibMemberRole::ExtensionBlob {
                requirement: ExtensionRequirement::Link,
                ..
            } => Some(LinkMemberRecord { record: self }),
            _ => None,
        }
    }
}

impl WireEncode for SlibMemberRecord {
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

#[derive(Clone, Copy, Debug)]
pub struct LinkMemberRecord<'record> {
    record: &'record SlibMemberRecord,
}

impl<'record> LinkMemberRecord<'record> {
    pub fn fingerprint(self) -> Result<LinkMemberFingerprint, HashError> {
        domain_separated_cbor_hash(LINK_MEMBER_FINGERPRINT_DOMAIN, self.record)
            .map(|digest| LinkMemberFingerprint(*digest.as_array()))
    }

    pub const fn record(self) -> &'record SlibMemberRecord {
        self.record
    }
}

impl SlibMemberId {
    pub(crate) fn from_stable_key(
        cone: ConeIdentity,
        stable_key: &MemberStableKey,
    ) -> Result<Self, HashError> {
        let mut stream = CanonicalHashStream::new();
        stream.update_byte_span(MEMBER_ID_DOMAIN.as_bytes())?;
        stream.update_raw(cone.as_array());
        stream.update_canonical_cbor(stable_key)?;
        Ok(Self(*stream.finalize().as_array()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlibMemberRecordError {
    KeyRoleMismatch,
    CapabilityMismatch,
    LengthOverflow,
    Hash(HashError),
}

impl fmt::Display for SlibMemberRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeyRoleMismatch => {
                formatter.write_str("member stable key and role variants do not match")
            }
            Self::CapabilityMismatch => {
                formatter.write_str("member stable key and role capabilities do not match")
            }
            Self::LengthOverflow => formatter.write_str("member payload length does not fit u64"),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SlibMemberRecordError {}

fn validate_key_role(
    stable_key: &MemberStableKey,
    role: &SlibMemberRole,
) -> Result<(), SlibMemberRecordError> {
    match (stable_key, role) {
        (MemberStableKey::HirMetadata, SlibMemberRole::HirMetadata)
        | (MemberStableKey::MirMetadata, SlibMemberRole::MirMetadata)
        | (MemberStableKey::LirMetadata, SlibMemberRole::LirMetadata) => Ok(()),
        (
            MemberStableKey::LinkObject {
                verifier_capability: key_capability,
                ..
            },
            SlibMemberRole::LinkObject {
                verifier_capability: role_capability,
                ..
            },
        )
        | (
            MemberStableKey::DiagnosticAttachment {
                capability: key_capability,
                ..
            },
            SlibMemberRole::DiagnosticAttachment {
                capability: role_capability,
            },
        )
        | (
            MemberStableKey::ExtensionBlob {
                capability: key_capability,
                ..
            },
            SlibMemberRole::ExtensionBlob {
                capability: role_capability,
                ..
            },
        ) => {
            if key_capability == role_capability {
                Ok(())
            } else {
                Err(SlibMemberRecordError::CapabilityMismatch)
            }
        }
        _ => Err(SlibMemberRecordError::KeyRoleMismatch),
    }
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_capability_key(
    encoder: &mut Encoder,
    tag: u64,
    capability: &scoop_identity::CapabilityId,
    logical_key: &LogicalMemberKey,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    capability.encode(encoder)?;
    encoder.field(2)?;
    encoder.bytes(logical_key.as_bytes())
}

fn encode_schema_role(
    encoder: &mut Encoder,
    tag: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    encoder.unsigned(WIRE_SCHEMA)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

#[cfg(test)]
mod tests;

mod decode;
pub use decode::*;
