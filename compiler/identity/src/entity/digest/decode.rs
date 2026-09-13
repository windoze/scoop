use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    DigestKind, DigestNodeKey, DigestNodeKeyError, DigestOwnerAndRoleKey, DigestPatchIntentKey,
    DigestSemanticFieldRole,
};
use crate::{
    ConeIdentity, DecodedObjectDefinitionPlanOwner, DecodedPersistentId, DefinitionAtomRole,
    DigestNodeId, ObjectDefinitionAtomId, ObjectDefinitionPlanId, ObjectDefinitionResolutionError,
    OdrGroupId, PersistentCallableBodyId, PersistentId, PersistentIdResolver, PersistentLayoutId,
    PersistentSafepointSiteId, PersistentScanId, StrongDefinitionResolver,
};

impl WireDecode for DigestKind {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::SourceSignature),
            2 => Ok(Self::Layout),
            3 => Ok(Self::Scan),
            4 => Ok(Self::LirDefinition),
            5 => Ok(Self::ObjectSupport),
            6 => Ok(Self::ObjectDefinition),
            7 => Ok(Self::StackmapRecord),
            8 => Ok(Self::OdrDefinition),
            9 => Ok(Self::StrongRegistration),
            10 => Ok(Self::RuntimeImage),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for DigestSemanticFieldRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::RegistrationDefinition),
            2 => Ok(Self::SourceSignature),
            3 => Ok(Self::Layout),
            4 => Ok(Self::Scan),
            5 => Ok(Self::DescriptorDefinition),
            6 => Ok(Self::CallableBodyDefinition),
            7 => Ok(Self::GatewayDefinition),
            8 => Ok(Self::NormalizedStackmap),
            9 => Ok(Self::RuntimeImage),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedDigestOwnerAndRoleKey {
    SourceSignature(DecodedPersistentId<PersistentCallableBodyId>),
    Layout(DecodedPersistentId<PersistentLayoutId>),
    Scan(DecodedPersistentId<PersistentScanId>),
    LirDefinition(DecodedPersistentId<ObjectDefinitionAtomId>),
    ObjectSupport(DecodedPersistentId<ObjectDefinitionAtomId>),
    ObjectDefinition(DecodedPersistentId<ObjectDefinitionAtomId>),
    StackmapRecord(DecodedPersistentId<PersistentSafepointSiteId>),
    OdrDefinition(DecodedPersistentId<OdrGroupId>),
    StrongRegistration(DecodedPersistentId<ObjectDefinitionPlanId>),
    RuntimeImage(DecodedPersistentId<ConeIdentity>),
}

impl DecodedDigestOwnerAndRoleKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DigestOwnerAndRoleKey, E>
    where
        R: DigestOwnerResolver<E>,
    {
        match self {
            Self::SourceSignature(id) => resolver
                .resolve(id)
                .map(DigestOwnerAndRoleKey::SourceSignature),
            Self::Layout(id) => resolver.resolve(id).map(DigestOwnerAndRoleKey::Layout),
            Self::Scan(id) => resolver.resolve(id).map(DigestOwnerAndRoleKey::Scan),
            Self::LirDefinition(id) => resolver
                .resolve(id)
                .map(DigestOwnerAndRoleKey::LirDefinition),
            Self::ObjectSupport(id) => resolver
                .resolve(id)
                .map(DigestOwnerAndRoleKey::ObjectSupport),
            Self::ObjectDefinition(id) => resolver
                .resolve(id)
                .map(DigestOwnerAndRoleKey::ObjectDefinition),
            Self::StackmapRecord(id) => resolver
                .resolve(id)
                .map(DigestOwnerAndRoleKey::StackmapRecord),
            Self::OdrDefinition(id) => resolver
                .resolve(id)
                .map(DigestOwnerAndRoleKey::OdrDefinition),
            Self::StrongRegistration(id) => resolver
                .resolve(id)
                .map(DigestOwnerAndRoleKey::StrongRegistration),
            Self::RuntimeImage(id) => resolver
                .resolve(id)
                .map(DigestOwnerAndRoleKey::RuntimeImage),
        }
    }
}

impl WireEncode for DecodedDigestOwnerAndRoleKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::SourceSignature(id) => encode_value_sum(encoder, 1, id),
            Self::Layout(id) => encode_value_sum(encoder, 2, id),
            Self::Scan(id) => encode_value_sum(encoder, 3, id),
            Self::LirDefinition(id) => encode_value_sum(encoder, 4, id),
            Self::ObjectSupport(id) => encode_value_sum(encoder, 5, id),
            Self::ObjectDefinition(id) => encode_value_sum(encoder, 6, id),
            Self::StackmapRecord(id) => encode_value_sum(encoder, 7, id),
            Self::OdrDefinition(id) => encode_value_sum(encoder, 8, id),
            Self::StrongRegistration(id) => encode_value_sum(encoder, 9, id),
            Self::RuntimeImage(id) => encode_value_sum(encoder, 10, id),
        }
    }
}

impl WireDecode for DecodedDigestOwnerAndRoleKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => decode_id_variant(decoder, fields, Self::SourceSignature),
            2 => decode_id_variant(decoder, fields, Self::Layout),
            3 => decode_id_variant(decoder, fields, Self::Scan),
            4 => decode_id_variant(decoder, fields, Self::LirDefinition),
            5 => decode_id_variant(decoder, fields, Self::ObjectSupport),
            6 => decode_id_variant(decoder, fields, Self::ObjectDefinition),
            7 => decode_id_variant(decoder, fields, Self::StackmapRecord),
            8 => decode_id_variant(decoder, fields, Self::OdrDefinition),
            9 => decode_id_variant(decoder, fields, Self::StrongRegistration),
            10 => decode_id_variant(decoder, fields, Self::RuntimeImage),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

pub trait DigestOwnerResolver<E>:
    PersistentIdResolver<PersistentCallableBodyId, Error = E>
    + PersistentIdResolver<PersistentLayoutId, Error = E>
    + PersistentIdResolver<PersistentScanId, Error = E>
    + PersistentIdResolver<ObjectDefinitionAtomId, Error = E>
    + PersistentIdResolver<PersistentSafepointSiteId, Error = E>
    + PersistentIdResolver<OdrGroupId, Error = E>
    + PersistentIdResolver<ObjectDefinitionPlanId, Error = E>
    + PersistentIdResolver<ConeIdentity, Error = E>
{
}

impl<T, E> DigestOwnerResolver<E> for T where
    T: PersistentIdResolver<PersistentCallableBodyId, Error = E>
        + PersistentIdResolver<PersistentLayoutId, Error = E>
        + PersistentIdResolver<PersistentScanId, Error = E>
        + PersistentIdResolver<ObjectDefinitionAtomId, Error = E>
        + PersistentIdResolver<PersistentSafepointSiteId, Error = E>
        + PersistentIdResolver<OdrGroupId, Error = E>
        + PersistentIdResolver<ObjectDefinitionPlanId, Error = E>
        + PersistentIdResolver<ConeIdentity, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedDigestNodeKey {
    kind: DigestKind,
    owner_and_role: DecodedDigestOwnerAndRoleKey,
}

impl DecodedDigestNodeKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DigestNodeKey, DigestNodeKeyResolutionError<E>>
    where
        R: DigestOwnerResolver<E>,
    {
        let owner_and_role = self
            .owner_and_role
            .resolve(resolver)
            .map_err(DigestNodeKeyResolutionError::Reference)?;
        DigestNodeKey::from_parts(self.kind, owner_and_role)
            .map_err(DigestNodeKeyResolutionError::Key)
    }
}

impl WireEncode for DecodedDigestNodeKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.owner_and_role.encode(encoder)
    }
}

impl WireDecode for DecodedDigestNodeKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            kind: decoder.field(1, DigestKind::decode)?,
            owner_and_role: decoder.field(2, DecodedDigestOwnerAndRoleKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DigestNodeKeyResolutionError<E> {
    Reference(E),
    Key(DigestNodeKeyError),
}

impl<E: fmt::Display> fmt::Display for DigestNodeKeyResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Key(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DigestNodeKeyResolutionError<E> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedDigestPatchIntentKey {
    source: DecodedPersistentId<DigestNodeId>,
    target_owner: DecodedObjectDefinitionPlanOwner,
    atom_role: DefinitionAtomRole,
    semantic_field_role: DigestSemanticFieldRole,
}

impl DecodedDigestPatchIntentKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DigestPatchIntentKey, DigestPatchIntentResolutionError<E>>
    where
        R: DigestPatchIntentResolver<E>,
    {
        let source = resolver
            .resolve(self.source)
            .map_err(DigestPatchIntentResolutionError::Source)?;
        let target_owner = self
            .target_owner
            .resolve(resolver)
            .map_err(DigestPatchIntentResolutionError::Target)?;
        Ok(DigestPatchIntentKey::new(
            source,
            target_owner,
            self.atom_role,
            self.semantic_field_role,
        ))
    }
}

impl WireEncode for DecodedDigestPatchIntentKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.source.encode(encoder)?;
        encoder.field(2)?;
        self.target_owner.encode(encoder)?;
        encoder.field(3)?;
        self.atom_role.encode(encoder)?;
        encoder.field(4)?;
        self.semantic_field_role.encode(encoder)
    }
}

impl WireDecode for DecodedDigestPatchIntentKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            source: decoder.field(1, DecodedPersistentId::decode)?,
            target_owner: decoder.field(2, DecodedObjectDefinitionPlanOwner::decode)?,
            atom_role: decoder.field(3, DefinitionAtomRole::decode)?,
            semantic_field_role: decoder.field(4, DigestSemanticFieldRole::decode)?,
        })
    }
}

pub trait DigestPatchIntentResolver<E>:
    PersistentIdResolver<DigestNodeId, Error = E> + StrongDefinitionResolver<E>
{
}

impl<T, E> DigestPatchIntentResolver<E> for T where
    T: PersistentIdResolver<DigestNodeId, Error = E> + StrongDefinitionResolver<E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DigestPatchIntentResolutionError<E> {
    Source(E),
    Target(ObjectDefinitionResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for DigestPatchIntentResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Target(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DigestPatchIntentResolutionError<E> {}

fn decode_sum_header(decoder: &mut Decoder<'_, '_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_, '_>,
    fields: u64,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: PersistentId,
{
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, DecodedPersistentId::decode).map(build)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests {
    use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

    use super::{DecodedDigestNodeKey, DecodedDigestPatchIntentKey, DigestNodeKeyResolutionError};
    use crate::{
        ConeIdentity, DefinitionAtomRole, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
        DigestSemanticFieldRole, ObjectDefinitionPlanOwner, PendingIdentityValidation,
        PersistentCallableBodyId, StrongDefinitionEntity,
    };

    #[test]
    fn node_and_patch_keys_resolve_all_typed_references() {
        let body = PersistentCallableBodyId(ConeIdentity::SINGLE_FILE.0);
        let node_key = DigestNodeKey::source_signature(body);
        let node = DigestNodeId::from_key(&node_key).unwrap();
        let target_owner = ObjectDefinitionPlanOwner::Strong {
            producer: ConeIdentity::SINGLE_FILE,
            entity: StrongDefinitionEntity::callable_body(body),
        };
        let patch_key = DigestPatchIntentKey::new(
            node,
            target_owner,
            DefinitionAtomRole::RuntimeRecord,
            DigestSemanticFieldRole::SourceSignature,
        );

        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(body).unwrap();
        pending
            .register_authority(ConeIdentity::SINGLE_FILE)
            .unwrap();
        pending.register_authority(node).unwrap();
        let mut identities = pending.finish().unwrap();

        let decoded_node = decode_canonical::<DecodedDigestNodeKey>(
            &encode(&node_key).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded_node.resolve(&mut identities).unwrap(), node_key);

        let decoded_patch = decode_canonical::<DecodedDigestPatchIntentKey>(
            &encode(&patch_key).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded_patch.resolve(&mut identities).unwrap(), patch_key);
    }

    #[test]
    fn node_resolution_rejects_independently_encoded_kind_owner_mismatch() {
        let body = PersistentCallableBodyId(ConeIdentity::SINGLE_FILE.0);
        let mut bytes = encode(&DigestNodeKey::source_signature(body)).unwrap();
        bytes[2] = 2;
        let decoded =
            decode_canonical::<DecodedDigestNodeKey>(&bytes, DecodeLimits::default()).unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(body).unwrap();
        let mut identities = pending.finish().unwrap();

        assert!(matches!(
            decoded.resolve(&mut identities),
            Err(DigestNodeKeyResolutionError::Key(_))
        ));
    }

    #[test]
    fn decoders_reject_unknown_closed_tags() {
        let error = decode_canonical::<DecodedDigestNodeKey>(
            &[0xa2, 0x01, 0x0b, 0x02, 0xa2, 0x00, 0x01, 0x01, 0x58, 0x20]
                .into_iter()
                .chain([0_u8; 32])
                .collect::<Vec<_>>(),
            DecodeLimits::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 11 });

        let body = PersistentCallableBodyId(ConeIdentity::SINGLE_FILE.0);
        let node_key = DigestNodeKey::source_signature(body);
        let node = DigestNodeId::from_key(&node_key).unwrap();
        let mut bytes = encode(&DigestPatchIntentKey::new(
            node,
            ObjectDefinitionPlanOwner::Strong {
                producer: ConeIdentity::SINGLE_FILE,
                entity: StrongDefinitionEntity::callable_body(body),
            },
            DefinitionAtomRole::RuntimeRecord,
            DigestSemanticFieldRole::SourceSignature,
        ))
        .unwrap();
        *bytes.last_mut().unwrap() = 10;
        let error =
            decode_canonical::<DecodedDigestPatchIntentKey>(&bytes, DecodeLimits::default())
                .unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 10 });
    }
}
