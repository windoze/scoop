//! Typed logical keys for the two built-in M23 link-object producers.

use std::fmt;
use std::num::NonZeroU32;

use scoop_identity::{
    CapabilityId, GeneratedBridgeUnitId, ObjectDefinitionPlanId, ObjectFormatId,
    TargetProfileWireId,
};
use scoop_wire::{
    Decoder, Digest256, Encoder, HashError, WireDecode, WireEncode, WireError,
    domain_separated_cbor_hash, encode,
};

use crate::{
    LogicalMemberKey, LogicalMemberKeyError, MemberStableKey, SlibMemberId, SlibMemberRole,
};

mod wire;
pub(crate) use wire::DecodedFixedBytesV1;
mod registration_identity;

const SCOOP_LIR_UNIT_SET_DOMAIN: &str = "scoop-lir-object-unit-set-v1";
const GENERATED_BRIDGE_UNIT_SET_DOMAIN: &str = "scoop-generated-bridge-object-unit-set-v1";

mod planning;
pub use planning::*;

mod c_bridge_production;
pub use c_bridge_production::*;

mod builtin_object_set;
pub use builtin_object_set::*;

mod digest_patch_sites;
pub use digest_patch_sites::*;

mod digest_fingerprints;
pub use digest_fingerprints::*;

mod odr_member_fingerprints;
mod shape_fingerprints;
pub use odr_member_fingerprints::{
    CallableDefinitionFingerprintV1, OdrMemberFingerprintV1, RegistrationFingerprintV1,
};
pub use shape_fingerprints::{OdrShapeFingerprintError, OdrShapeFingerprintV1};

mod odr_directory;
pub use odr_directory::*;

mod stackmap_normalization;
pub use stackmap_normalization::*;

mod safepoint_registrations;
pub use safepoint_registrations::*;

mod callable_registrations;
pub use callable_registrations::*;

mod type_registrations;
pub use type_registrations::*;

mod immortal_registrations;
pub use immortal_registrations::*;

mod static_storage_registrations;
pub use static_storage_registrations::*;

mod initialization_registrations;
pub use initialization_registrations::*;

mod cone_image;
pub use cone_image::*;

mod entry_production;
pub use entry_production::*;

mod strong_registration_finalization;
pub use strong_registration_finalization::*;

mod registration_projection;
pub use registration_projection::*;

mod generated_bridge_semantics;
pub use generated_bridge_semantics::*;

mod symbol_planning;
pub use symbol_planning::*;

mod symbol_verification;
pub use symbol_verification::*;

mod relocation_verification;
pub use relocation_verification::*;

mod strong_relocation_closure;
pub use strong_relocation_closure::*;

mod current_cone_requirements;
pub use current_cone_requirements::*;

mod cross_cone_link_closure;
pub use cross_cone_link_closure::*;

mod layout_link_closure;
pub use layout_link_closure::*;

mod native_requirements;
pub use native_requirements::*;

mod runtime_requirements;
pub use runtime_requirements::*;

mod c_bridge_target_requirements;
pub use c_bridge_target_requirements::*;

mod undefined_requirements;
pub use undefined_requirements::*;

mod defined_owners;
pub use defined_owners::*;

mod code_fingerprint;
pub use code_fingerprint::*;

mod layout_code_fingerprint;
pub use layout_code_fingerprint::*;

mod link_identity_closure;
pub use link_identity_closure::*;

mod macho;
pub use macho::*;

mod elf;
pub use elf::{ElfObjectError, ElfRelocation, ValidatedElfObject};

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

typed_digest!(ScoopLirObjectUnitSetDigest);
typed_digest!(GeneratedBridgeObjectUnitSetDigest);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalScoopLirObjectUnitSetV1 {
    units: Vec<ObjectDefinitionPlanId>,
    logical_key: ScoopLirObjectLogicalKeyV1,
}

impl CanonicalScoopLirObjectUnitSetV1 {
    pub fn new(mut units: Vec<ObjectDefinitionPlanId>) -> Result<Self, ObjectUnitSetError> {
        canonicalize_units(&mut units, ObjectUnitKind::ScoopLir)?;
        let unit_count = checked_unit_count(units.len(), ObjectUnitKind::ScoopLir)?;
        let digest =
            domain_separated_cbor_hash(SCOOP_LIR_UNIT_SET_DOMAIN, &CanonicalUnitSequence(&units))
                .map_err(ObjectUnitSetError::Hash)?;
        Ok(Self {
            units,
            logical_key: ScoopLirObjectLogicalKeyV1 {
                unit_count,
                unit_set_digest: ScoopLirObjectUnitSetDigest(*digest.as_array()),
            },
        })
    }

    pub fn units(&self) -> &[ObjectDefinitionPlanId] {
        &self.units
    }

    pub const fn logical_key(&self) -> ScoopLirObjectLogicalKeyV1 {
        self.logical_key
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalGeneratedBridgeObjectUnitSetV1 {
    units: Vec<GeneratedBridgeUnitId>,
    logical_key: GeneratedBridgeObjectLogicalKeyV1,
}

impl CanonicalGeneratedBridgeObjectUnitSetV1 {
    pub fn new(mut units: Vec<GeneratedBridgeUnitId>) -> Result<Self, ObjectUnitSetError> {
        canonicalize_units(&mut units, ObjectUnitKind::GeneratedBridge)?;
        let unit_count = checked_unit_count(units.len(), ObjectUnitKind::GeneratedBridge)?;
        let digest = domain_separated_cbor_hash(
            GENERATED_BRIDGE_UNIT_SET_DOMAIN,
            &CanonicalUnitSequence(&units),
        )
        .map_err(ObjectUnitSetError::Hash)?;
        Ok(Self {
            units,
            logical_key: GeneratedBridgeObjectLogicalKeyV1 {
                unit_count,
                unit_set_digest: GeneratedBridgeObjectUnitSetDigest(*digest.as_array()),
            },
        })
    }

    pub fn units(&self) -> &[GeneratedBridgeUnitId] {
        &self.units
    }

    pub const fn logical_key(&self) -> GeneratedBridgeObjectLogicalKeyV1 {
        self.logical_key
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedScoopLirObjectMemberV1 {
    member_id: SlibMemberId,
    stable_key: MemberStableKey,
    role: SlibMemberRole,
    units: CanonicalScoopLirObjectUnitSetV1,
}

impl PlannedScoopLirObjectMemberV1 {
    pub fn new(
        cone: scoop_identity::ConeIdentity,
        units: CanonicalScoopLirObjectUnitSetV1,
    ) -> Result<Self, LinkObjectMemberPlanError> {
        let stable_key = units
            .logical_key()
            .member_stable_key()
            .map_err(LinkObjectMemberPlanError::LogicalKey)?;
        let member_id = SlibMemberId::from_stable_key(cone, &stable_key)
            .map_err(LinkObjectMemberPlanError::MemberIdentity)?;
        Ok(Self {
            member_id,
            stable_key,
            role: units.logical_key().member_role(),
            units,
        })
    }

    pub const fn member_id(&self) -> SlibMemberId {
        self.member_id
    }

    pub const fn stable_key(&self) -> &MemberStableKey {
        &self.stable_key
    }

    pub const fn role(&self) -> &SlibMemberRole {
        &self.role
    }

    pub const fn units(&self) -> &CanonicalScoopLirObjectUnitSetV1 {
        &self.units
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedGeneratedBridgeObjectMemberV1 {
    member_id: SlibMemberId,
    stable_key: MemberStableKey,
    role: SlibMemberRole,
    units: CanonicalGeneratedBridgeObjectUnitSetV1,
}

impl PlannedGeneratedBridgeObjectMemberV1 {
    pub fn new(
        cone: scoop_identity::ConeIdentity,
        units: CanonicalGeneratedBridgeObjectUnitSetV1,
    ) -> Result<Self, LinkObjectMemberPlanError> {
        let stable_key = units
            .logical_key()
            .member_stable_key()
            .map_err(LinkObjectMemberPlanError::LogicalKey)?;
        let member_id = SlibMemberId::from_stable_key(cone, &stable_key)
            .map_err(LinkObjectMemberPlanError::MemberIdentity)?;
        Ok(Self {
            member_id,
            stable_key,
            role: units.logical_key().member_role(),
            units,
        })
    }

    pub const fn member_id(&self) -> SlibMemberId {
        self.member_id
    }

    pub const fn stable_key(&self) -> &MemberStableKey {
        &self.stable_key
    }

    pub const fn role(&self) -> &SlibMemberRole {
        &self.role
    }

    pub const fn units(&self) -> &CanonicalGeneratedBridgeObjectUnitSetV1 {
        &self.units
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoopLirObjectLogicalKeyV1 {
    unit_count: NonZeroU32,
    unit_set_digest: ScoopLirObjectUnitSetDigest,
}

impl ScoopLirObjectLogicalKeyV1 {
    pub const fn unit_count(self) -> NonZeroU32 {
        self.unit_count
    }

    pub const fn unit_set_digest(self) -> ScoopLirObjectUnitSetDigest {
        self.unit_set_digest
    }

    pub fn member_stable_key(self) -> Result<MemberStableKey, ObjectLogicalKeyError> {
        encode_logical_key(&self).map(|logical_key| MemberStableKey::LinkObject {
            verifier_capability: scoop_lir_link_object_capability(),
            logical_key,
        })
    }

    pub fn member_role(self) -> SlibMemberRole {
        link_object_role(scoop_lir_link_object_capability())
    }
}

impl WireEncode for ScoopLirObjectLogicalKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_object_logical_key(encoder, self.unit_count, &self.unit_set_digest)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedBridgeObjectLogicalKeyV1 {
    unit_count: NonZeroU32,
    unit_set_digest: GeneratedBridgeObjectUnitSetDigest,
}

impl GeneratedBridgeObjectLogicalKeyV1 {
    pub const fn unit_count(self) -> NonZeroU32 {
        self.unit_count
    }

    pub const fn unit_set_digest(self) -> GeneratedBridgeObjectUnitSetDigest {
        self.unit_set_digest
    }

    pub fn member_stable_key(self) -> Result<MemberStableKey, ObjectLogicalKeyError> {
        encode_logical_key(&self).map(|logical_key| MemberStableKey::LinkObject {
            verifier_capability: generated_c_bridge_link_object_capability(),
            logical_key,
        })
    }

    pub fn member_role(self) -> SlibMemberRole {
        link_object_role(generated_c_bridge_link_object_capability())
    }
}

impl WireEncode for GeneratedBridgeObjectLogicalKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_object_logical_key(encoder, self.unit_count, &self.unit_set_digest)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedScoopLirObjectLogicalKeyV1 {
    unit_count: u32,
    unit_set_digest: Digest256,
}

impl DecodedScoopLirObjectLogicalKeyV1 {
    pub fn validate(
        self,
        expected: &CanonicalScoopLirObjectUnitSetV1,
    ) -> Result<ScoopLirObjectLogicalKeyV1, ObjectLogicalKeyValidationError> {
        validate_decoded_logical_key(
            self.unit_count,
            self.unit_set_digest,
            expected.logical_key().unit_count,
            expected.logical_key().unit_set_digest.as_array(),
        )?;
        Ok(expected.logical_key())
    }
}

impl WireEncode for DecodedScoopLirObjectLogicalKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_decoded_object_logical_key(encoder, self.unit_count, &self.unit_set_digest)
    }
}

impl WireDecode for DecodedScoopLirObjectLogicalKeyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decode_object_logical_key(decoder).map(|(unit_count, unit_set_digest)| Self {
            unit_count,
            unit_set_digest,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedGeneratedBridgeObjectLogicalKeyV1 {
    unit_count: u32,
    unit_set_digest: Digest256,
}

impl DecodedGeneratedBridgeObjectLogicalKeyV1 {
    pub fn validate(
        self,
        expected: &CanonicalGeneratedBridgeObjectUnitSetV1,
    ) -> Result<GeneratedBridgeObjectLogicalKeyV1, ObjectLogicalKeyValidationError> {
        validate_decoded_logical_key(
            self.unit_count,
            self.unit_set_digest,
            expected.logical_key().unit_count,
            expected.logical_key().unit_set_digest.as_array(),
        )?;
        Ok(expected.logical_key())
    }
}

impl WireEncode for DecodedGeneratedBridgeObjectLogicalKeyV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_decoded_object_logical_key(encoder, self.unit_count, &self.unit_set_digest)
    }
}

impl WireDecode for DecodedGeneratedBridgeObjectLogicalKeyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decode_object_logical_key(decoder).map(|(unit_count, unit_set_digest)| Self {
            unit_count,
            unit_set_digest,
        })
    }
}

pub fn scoop_lir_link_object_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.link-object", "scoop-lir", 5)
        .expect("built-in Scoop LIR object capability is valid")
}

pub fn generated_c_bridge_link_object_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.link-object", "generated-c-bridge", 1)
        .expect("built-in generated C bridge object capability is valid")
}

fn link_object_role(verifier_capability: CapabilityId) -> SlibMemberRole {
    SlibMemberRole::LinkObject {
        target_profile: TargetProfileWireId::darwin_aarch64(),
        object_format: ObjectFormatId::macho_relocatable(),
        verifier_capability,
    }
}

struct CanonicalUnitSequence<'units, I>(&'units [I]);

impl<I: WireEncode> WireEncode for CanonicalUnitSequence<'_, I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for unit in self.0 {
            unit.encode(encoder)?;
        }
        Ok(())
    }
}

fn canonicalize_units<I: Copy + Ord>(
    units: &mut [I],
    kind: ObjectUnitKind,
) -> Result<(), ObjectUnitSetError> {
    if units.is_empty() {
        return Err(ObjectUnitSetError::Empty(kind));
    }
    units.sort_unstable();
    if units.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ObjectUnitSetError::Duplicate(kind));
    }
    Ok(())
}

fn checked_unit_count(
    length: usize,
    kind: ObjectUnitKind,
) -> Result<NonZeroU32, ObjectUnitSetError> {
    let count =
        u32::try_from(length).map_err(|_| ObjectUnitSetError::TooMany { actual: length })?;
    NonZeroU32::new(count).ok_or(ObjectUnitSetError::Empty(kind))
}

fn encode_object_logical_key(
    encoder: &mut Encoder,
    unit_count: NonZeroU32,
    digest: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    encoder.unsigned(u64::from(unit_count.get()))?;
    encoder.field(2)?;
    digest.encode(encoder)
}

fn encode_decoded_object_logical_key(
    encoder: &mut Encoder,
    unit_count: u32,
    digest: &Digest256,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    encoder.unsigned(u64::from(unit_count))?;
    encoder.field(2)?;
    digest.encode(encoder)
}

fn decode_object_logical_key(decoder: &mut Decoder<'_>) -> Result<(u32, Digest256), WireError> {
    decoder.expect_map(2)?;
    Ok((
        decoder.field(1, Decoder::u32)?,
        decoder.field(2, Digest256::decode)?,
    ))
}

fn encode_logical_key(value: &impl WireEncode) -> Result<LogicalMemberKey, ObjectLogicalKeyError> {
    let bytes = encode(value).map_err(ObjectLogicalKeyError::Encode)?;
    LogicalMemberKey::new(bytes).map_err(ObjectLogicalKeyError::LogicalKey)
}

fn validate_decoded_logical_key(
    actual_count: u32,
    actual_digest: Digest256,
    expected_count: NonZeroU32,
    expected_digest: &[u8; 32],
) -> Result<(), ObjectLogicalKeyValidationError> {
    if actual_count == 0 {
        return Err(ObjectLogicalKeyValidationError::ZeroUnitCount);
    }
    if actual_count != expected_count.get() {
        return Err(ObjectLogicalKeyValidationError::UnitCountMismatch {
            expected: expected_count.get(),
            actual: actual_count,
        });
    }
    if actual_digest.as_array() != expected_digest {
        return Err(ObjectLogicalKeyValidationError::UnitSetDigestMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectUnitKind {
    ScoopLir,
    GeneratedBridge,
}

impl fmt::Display for ObjectUnitKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ScoopLir => "Scoop LIR definition plan",
            Self::GeneratedBridge => "generated C bridge unit",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectUnitSetError {
    Empty(ObjectUnitKind),
    Duplicate(ObjectUnitKind),
    TooMany { actual: usize },
    Hash(HashError),
}

impl fmt::Display for ObjectUnitSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(kind) => write!(formatter, "{kind} set must not be empty"),
            Self::Duplicate(kind) => write!(formatter, "{kind} set contains a duplicate unit"),
            Self::TooMany { actual } => write!(
                formatter,
                "object unit set has {actual} entries, exceeding u32::MAX"
            ),
            Self::Hash(error) => write!(formatter, "cannot hash object unit set: {error}"),
        }
    }
}

impl std::error::Error for ObjectUnitSetError {}

#[derive(Debug)]
pub enum ObjectLogicalKeyError {
    Encode(scoop_wire::cbor::EncodeError),
    LogicalKey(LogicalMemberKeyError),
}

impl fmt::Display for ObjectLogicalKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => write!(formatter, "cannot encode object logical key: {error}"),
            Self::LogicalKey(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ObjectLogicalKeyError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectLogicalKeyValidationError {
    ZeroUnitCount,
    UnitCountMismatch { expected: u32, actual: u32 },
    UnitSetDigestMismatch,
}

impl fmt::Display for ObjectLogicalKeyValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroUnitCount => formatter.write_str("object logical key has zero units"),
            Self::UnitCountMismatch { expected, actual } => write!(
                formatter,
                "object logical key unit count mismatch: expected {expected}, found {actual}"
            ),
            Self::UnitSetDigestMismatch => {
                formatter.write_str("object logical key unit-set digest mismatch")
            }
        }
    }
}

impl std::error::Error for ObjectLogicalKeyValidationError {}

#[derive(Debug)]
pub enum LinkObjectMemberPlanError {
    LogicalKey(ObjectLogicalKeyError),
    MemberIdentity(HashError),
}

impl fmt::Display for LinkObjectMemberPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LogicalKey(error) => error.fmt(formatter),
            Self::MemberIdentity(error) => {
                write!(
                    formatter,
                    "cannot derive link-object member identity: {error}"
                )
            }
        }
    }
}

impl std::error::Error for LinkObjectMemberPlanError {}

#[cfg(test)]
mod tests;
