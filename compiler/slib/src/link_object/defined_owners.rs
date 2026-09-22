//! Canonical member-aware owners for all verified external strong definitions.

use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedPersistentId, DecodedStrongDefinitionEntity, GeneratedBridgeAtomId,
    ObjectDefinitionAtomId, ObjectDefinitionPlanKey, StrongDefinitionEntity,
    StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

use super::{
    PlannedStrongObjectSymbolRoleV1, VerifiedBoundaryRoleV1,
    VerifiedCurrentConeStrongRelocationClosureV1,
};
use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StrongDefinitionOwnerV1 {
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
}

impl StrongDefinitionOwnerV1 {
    pub fn new(
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    ) -> Result<Self, StrongDefinitionOwnerValidationError> {
        ObjectDefinitionPlanKey::strong(ConeIdentity::CORE, entity, role).map_err(|_| {
            StrongDefinitionOwnerValidationError::EntityRoleMismatch { entity, role }
        })?;
        if matches!(
            entity.kind(),
            StrongDefinitionEntityKind::GeneratedBridgeAtom(_)
                | StrongDefinitionEntityKind::ConeImage(_)
        ) {
            return Err(StrongDefinitionOwnerValidationError::ReservedOwnerKind { entity, role });
        }
        Ok(Self { entity, role })
    }

    pub const fn entity(self) -> StrongDefinitionEntity {
        self.entity
    }

    pub const fn role(self) -> StrongDefinitionRole {
        self.role
    }
}

impl WireEncode for StrongDefinitionOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.entity.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongDefinitionOwnerValidationError {
    EntityRoleMismatch {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
    ReservedOwnerKind {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
}

impl fmt::Display for StrongDefinitionOwnerValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong definition owner: {self:?}")
    }
}

impl std::error::Error for StrongDefinitionOwnerValidationError {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LinkDefinitionOwnerV1 {
    StrongDefinition(StrongDefinitionOwnerV1),
    GeneratedBridge(GeneratedBridgeAtomId),
    ConeImage(ConeIdentity),
    VerifierBoundary {
        atom: ObjectDefinitionAtomId,
        boundary: VerifiedBoundaryRoleV1,
    },
}

impl WireEncode for LinkDefinitionOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::StrongDefinition(owner) => encode_one_field_sum(encoder, 1, owner),
            Self::GeneratedBridge(atom) => encode_one_field_sum(encoder, 2, atom),
            Self::ConeImage(cone) => encode_one_field_sum(encoder, 3, cone),
            Self::VerifierBoundary { atom, boundary } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                atom.encode(encoder)?;
                encoder.field(2)?;
                encode_boundary(encoder, *boundary)
            }
        }
    }
}

impl LinkDefinitionOwnerV1 {
    pub fn from_strong_primary(
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    ) -> Result<Self, StrongDefinitionOwnerValidationError> {
        match (entity.kind(), role) {
            (
                StrongDefinitionEntityKind::GeneratedBridgeAtom(atom),
                StrongDefinitionRole::GeneratedBridge,
            ) => Ok(Self::GeneratedBridge(atom)),
            (
                StrongDefinitionEntityKind::ConeImage(cone),
                StrongDefinitionRole::ImageDescriptor,
            ) => Ok(Self::ConeImage(cone)),
            _ => StrongDefinitionOwnerV1::new(entity, role).map(Self::StrongDefinition),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinedLinkSymbolOwnerV1 {
    member: SlibMemberId,
    symbol: Vec<u8>,
    owner: LinkDefinitionOwnerV1,
}

impl WireEncode for DefinedLinkSymbolOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        encoder.bytes(&self.symbol)?;
        encoder.field(3)?;
        self.owner.encode(encoder)
    }
}

impl DefinedLinkSymbolOwnerV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }

    pub fn symbol(&self) -> &[u8] {
        &self.symbol
    }

    pub const fn owner(&self) -> LinkDefinitionOwnerV1 {
        self.owner
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDefinedLinkSymbolOwnerSetV1 {
    producer: ConeIdentity,
    owners: Vec<DefinedLinkSymbolOwnerV1>,
}

impl CanonicalDefinedLinkSymbolOwnerSetV1 {
    pub fn from_verified_strong_closure(
        closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    ) -> Result<Self, DefinedLinkSymbolOwnerBuildError> {
        let mut primary_owners = BTreeSet::new();
        let mut owners = Vec::new();
        for member in closure.members() {
            for symbol in member.definitions().symbols() {
                let owner = match symbol.role() {
                    PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                        owner,
                        definition_role,
                        ..
                    } => {
                        let link_owner = primary_owner(owner, definition_role)?;
                        if !primary_owners.insert(link_owner) {
                            return Err(DefinedLinkSymbolOwnerBuildError::DuplicatePrimaryOwner(
                                link_owner,
                            ));
                        }
                        link_owner
                    }
                    PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { atom, .. } => {
                        LinkDefinitionOwnerV1::VerifierBoundary {
                            atom,
                            boundary: VerifiedBoundaryRoleV1::Start,
                        }
                    }
                    PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { atom, .. } => {
                        LinkDefinitionOwnerV1::VerifierBoundary {
                            atom,
                            boundary: VerifiedBoundaryRoleV1::End,
                        }
                    }
                };
                owners.push(DefinedLinkSymbolOwnerV1 {
                    member: member.member(),
                    symbol: symbol.macho_name().to_vec(),
                    owner,
                });
            }
        }
        owners.sort_unstable_by(|left, right| {
            (&left.symbol, left.member, left.owner).cmp(&(&right.symbol, right.member, right.owner))
        });
        if let Some(pair) = owners
            .windows(2)
            .find(|pair| pair[0].symbol == pair[1].symbol)
        {
            return Err(DefinedLinkSymbolOwnerBuildError::DuplicateSymbol(
                pair[0].symbol.clone(),
            ));
        }
        Ok(Self {
            producer: closure.producer(),
            owners,
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn owners(&self) -> &[DefinedLinkSymbolOwnerV1] {
        &self.owners
    }

    #[cfg(test)]
    pub(in crate::link_object) fn replace_owner_for_test(
        &self,
        symbol: &[u8],
        owner: LinkDefinitionOwnerV1,
    ) -> Self {
        let mut changed = self.clone();
        let record = changed
            .owners
            .iter_mut()
            .find(|record| record.symbol() == symbol)
            .expect("test symbol belongs to the verified owner set");
        record.owner = owner;
        changed.owners.sort_unstable_by(|left, right| {
            (&left.symbol, left.member, left.owner).cmp(&(&right.symbol, right.member, right.owner))
        });
        changed
    }

    #[cfg(test)]
    pub(in crate::link_object) fn insert_foreign_owner_for_test(
        &self,
        symbol: Vec<u8>,
        owner: LinkDefinitionOwnerV1,
    ) -> Self {
        let mut changed = self.clone();
        changed.owners.push(DefinedLinkSymbolOwnerV1 {
            member: changed.owners[0].member,
            symbol,
            owner,
        });
        changed.owners.sort_unstable_by(|left, right| {
            (&left.symbol, left.member, left.owner).cmp(&(&right.symbol, right.member, right.owner))
        });
        changed
    }
}

impl WireEncode for CanonicalDefinedLinkSymbolOwnerSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.owners.len() as u64)?;
        for owner in &self.owners {
            owner.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedMemberId([u8; 32]);

impl WireEncode for DecodedMemberId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl WireDecode for DecodedMemberId {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decode_fixed_32(decoder).map(Self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DecodedStrongDefinitionOwnerV1 {
    entity: DecodedStrongDefinitionEntity,
    role: StrongDefinitionRole,
}

impl WireEncode for DecodedStrongDefinitionOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.entity.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl WireDecode for DecodedStrongDefinitionOwnerV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            entity: decoder.field(1, DecodedStrongDefinitionEntity::decode)?,
            role: decoder.field(2, StrongDefinitionRole::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedLinkDefinitionOwnerV1 {
    StrongDefinition(DecodedStrongDefinitionOwnerV1),
    GeneratedBridge(DecodedPersistentId<GeneratedBridgeAtomId>),
    ConeImage(DecodedPersistentId<ConeIdentity>),
    VerifierBoundary {
        atom: DecodedPersistentId<ObjectDefinitionAtomId>,
        boundary: VerifiedBoundaryRoleV1,
    },
}

impl WireEncode for DecodedLinkDefinitionOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::StrongDefinition(owner) => encode_one_field_sum(encoder, 1, owner),
            Self::GeneratedBridge(atom) => encode_one_field_sum(encoder, 2, atom),
            Self::ConeImage(cone) => encode_one_field_sum(encoder, 3, cone),
            Self::VerifierBoundary { atom, boundary } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                atom.encode(encoder)?;
                encoder.field(2)?;
                encode_boundary(encoder, *boundary)
            }
        }
    }
}

impl WireDecode for DecodedLinkDefinitionOwnerV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedStrongDefinitionOwnerV1::decode)
                    .map(Self::StrongDefinition)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::GeneratedBridge)
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::ConeImage)
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::VerifierBoundary {
                    atom: decoder.field(1, DecodedPersistentId::decode)?,
                    boundary: decoder.field(2, decode_boundary)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedDefinedLinkSymbolOwnerV1 {
    member: DecodedMemberId,
    symbol: Vec<u8>,
    owner: DecodedLinkDefinitionOwnerV1,
}

impl WireEncode for DecodedDefinedLinkSymbolOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.member.encode(encoder)?;
        encoder.field(2)?;
        encoder.bytes(&self.symbol)?;
        encoder.field(3)?;
        self.owner.encode(encoder)
    }
}

impl WireDecode for DecodedDefinedLinkSymbolOwnerV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            member: decoder.field(1, DecodedMemberId::decode)?,
            symbol: decoder.field(2, Decoder::owned_bytes)?,
            owner: decoder.field(3, DecodedLinkDefinitionOwnerV1::decode)?,
        })
    }
}

/// Untrusted wire projection. It can only be promoted by comparing it with
/// the set rebuilt from verified object definitions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalDefinedLinkSymbolOwnerSetV1 {
    owners: Vec<DecodedDefinedLinkSymbolOwnerV1>,
}

impl DecodedCanonicalDefinedLinkSymbolOwnerSetV1 {
    pub fn validate_against(
        self,
        expected: &CanonicalDefinedLinkSymbolOwnerSetV1,
    ) -> Result<CanonicalDefinedLinkSymbolOwnerSetV1, DefinedLinkSymbolOwnerValidationError> {
        let actual = encode(&self).map_err(DefinedLinkSymbolOwnerValidationError::Encode)?;
        let expected_bytes =
            encode(expected).map_err(DefinedLinkSymbolOwnerValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(DefinedLinkSymbolOwnerValidationError::ProjectionMismatch);
        }
        Ok(expected.clone())
    }
}

impl WireEncode for DecodedCanonicalDefinedLinkSymbolOwnerSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.owners.len() as u64)?;
        for owner in &self.owners {
            owner.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalDefinedLinkSymbolOwnerSetV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedDefinedLinkSymbolOwnerV1::decode(decoder))
            .map(|owners| Self { owners })
    }
}

fn primary_owner(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> Result<LinkDefinitionOwnerV1, DefinedLinkSymbolOwnerBuildError> {
    LinkDefinitionOwnerV1::from_strong_primary(entity, role)
        .map_err(|_| DefinedLinkSymbolOwnerBuildError::EntityRoleMismatch { entity, role })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinedLinkSymbolOwnerBuildError {
    EntityRoleMismatch {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
    DuplicatePrimaryOwner(LinkDefinitionOwnerV1),
    DuplicateSymbol(Vec<u8>),
}

impl fmt::Display for DefinedLinkSymbolOwnerBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid defined link-symbol owner set: {self:?}")
    }
}

impl std::error::Error for DefinedLinkSymbolOwnerBuildError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinedLinkSymbolOwnerValidationError {
    ProjectionMismatch,
    Encode(scoop_wire::cbor::EncodeError),
}

impl fmt::Display for DefinedLinkSymbolOwnerValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid defined link-symbol owner section: {self:?}"
        )
    }
}

impl std::error::Error for DefinedLinkSymbolOwnerValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::ProjectionMismatch => None,
        }
    }
}

fn encode_boundary(
    encoder: &mut Encoder,
    boundary: VerifiedBoundaryRoleV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.unsigned(match boundary {
        VerifiedBoundaryRoleV1::Start => 1,
        VerifiedBoundaryRoleV1::End => 2,
    })
}

fn decode_boundary(decoder: &mut Decoder<'_, '_>) -> Result<VerifiedBoundaryRoleV1, WireError> {
    match decoder.unsigned()? {
        1 => Ok(VerifiedBoundaryRoleV1::Start),
        2 => Ok(VerifiedBoundaryRoleV1::End),
        tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    }
}

fn encode_one_field_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn decode_fixed_32(decoder: &mut Decoder<'_, '_>) -> Result<[u8; 32], WireError> {
    let bytes = decoder.bytes()?;
    <&[u8; 32]>::try_from(bytes).copied().map_err(|_| {
        wire_error(
            decoder,
            WireErrorKind::InvalidLength {
                expected: 32,
                actual: bytes.len() as u64,
            },
        )
    })
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

#[cfg(test)]
mod tests;
