use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{OdrIdentityResolutionError, OdrMemberResolver};
use crate::{
    DecodedPersistentId, OdrGroupId, OdrMemberDiscriminator, OdrMemberKey, OdrMemberRole,
    PersistentCallableApplicationId, PersistentCallableBodyId, PersistentDispatchSlotId,
    PersistentDispatchTableId, PersistentExactTypeId, PersistentGeneratedCallableId,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentLayoutId,
    PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId, PersistentTypeId,
    StructuralDefinitionPath,
};

impl WireDecode for OdrMemberRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::CallableBody),
            2 => Ok(Self::GeneratedNominal),
            3 => Ok(Self::Layout),
            4 => Ok(Self::ScanProgram),
            5 => Ok(Self::TypeDescriptor),
            6 => Ok(Self::DispatchTable),
            7 => Ok(Self::DispatchAdapter),
            8 => Ok(Self::StaticStorage),
            9 => Ok(Self::ImmortalObject),
            10 => Ok(Self::InitializationCell),
            12 => Ok(Self::RegistrationRecord),
            13 => Ok(Self::DiagnosticBytes),
            14 => Ok(Self::AddressTakenConstant),
            15 => Ok(Self::ObjectSupport),
            16 => Ok(Self::ReleaseHook),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedOdrMemberDiscriminator {
    Singleton,
    CallableApplication(DecodedPersistentId<PersistentCallableApplicationId>),
    GeneratedCallable(DecodedPersistentId<PersistentGeneratedCallableId>),
    GeneratedNominal(DecodedPersistentId<PersistentTypeId>),
    ExactType(DecodedPersistentId<PersistentExactTypeId>),
    Layout(DecodedPersistentId<PersistentLayoutId>),
    Scan(DecodedPersistentId<PersistentScanId>),
    DispatchTable(DecodedPersistentId<PersistentDispatchTableId>),
    DispatchSlot(DecodedPersistentId<PersistentDispatchSlotId>),
    StaticStorage(DecodedPersistentId<PersistentStaticStorageId>),
    ImmortalObject(DecodedPersistentId<PersistentImmortalObjectId>),
    InitializationUnit(DecodedPersistentId<PersistentInitializationUnitId>),
    StructuralPath(StructuralDefinitionPath),
    CallableBody(DecodedPersistentId<PersistentCallableBodyId>),
    SafepointSite(DecodedPersistentId<PersistentSafepointSiteId>),
}

impl DecodedOdrMemberDiscriminator {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<OdrMemberDiscriminator, E>
    where
        R: OdrMemberResolver<E>,
    {
        match self {
            Self::Singleton => Ok(OdrMemberDiscriminator::Singleton),
            Self::CallableApplication(id) => resolver
                .resolve(id)
                .map(OdrMemberDiscriminator::CallableApplication),
            Self::GeneratedCallable(id) => resolver
                .resolve(id)
                .map(OdrMemberDiscriminator::GeneratedCallable),
            Self::GeneratedNominal(id) => resolver
                .resolve(id)
                .map(OdrMemberDiscriminator::GeneratedNominal),
            Self::ExactType(id) => resolver.resolve(id).map(OdrMemberDiscriminator::ExactType),
            Self::Layout(id) => resolver.resolve(id).map(OdrMemberDiscriminator::Layout),
            Self::Scan(id) => resolver.resolve(id).map(OdrMemberDiscriminator::Scan),
            Self::DispatchTable(id) => resolver
                .resolve(id)
                .map(OdrMemberDiscriminator::DispatchTable),
            Self::DispatchSlot(id) => resolver
                .resolve(id)
                .map(OdrMemberDiscriminator::DispatchSlot),
            Self::StaticStorage(id) => resolver
                .resolve(id)
                .map(OdrMemberDiscriminator::StaticStorage),
            Self::ImmortalObject(id) => resolver
                .resolve(id)
                .map(OdrMemberDiscriminator::ImmortalObject),
            Self::InitializationUnit(id) => resolver
                .resolve(id)
                .map(OdrMemberDiscriminator::InitializationUnit),
            Self::StructuralPath(path) => Ok(OdrMemberDiscriminator::StructuralPath(path)),
            Self::CallableBody(id) => resolver
                .resolve(id)
                .map(OdrMemberDiscriminator::CallableBody),
            Self::SafepointSite(id) => resolver
                .resolve(id)
                .map(OdrMemberDiscriminator::SafepointSite),
        }
    }
}

impl WireEncode for DecodedOdrMemberDiscriminator {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Singleton => encode_empty_sum(encoder, 1),
            Self::CallableApplication(id) => encode_value_sum(encoder, 2, id),
            Self::GeneratedCallable(id) => encode_value_sum(encoder, 3, id),
            Self::GeneratedNominal(id) => encode_value_sum(encoder, 4, id),
            Self::ExactType(id) => encode_value_sum(encoder, 5, id),
            Self::Layout(id) => encode_value_sum(encoder, 6, id),
            Self::Scan(id) => encode_value_sum(encoder, 7, id),
            Self::DispatchTable(id) => encode_value_sum(encoder, 8, id),
            Self::DispatchSlot(id) => encode_value_sum(encoder, 9, id),
            Self::StaticStorage(id) => encode_value_sum(encoder, 10, id),
            Self::ImmortalObject(id) => encode_value_sum(encoder, 11, id),
            Self::InitializationUnit(id) => encode_value_sum(encoder, 12, id),
            Self::StructuralPath(path) => encode_value_sum(encoder, 13, path),
            Self::CallableBody(id) => encode_value_sum(encoder, 14, id),
            Self::SafepointSite(id) => encode_value_sum(encoder, 15, id),
        }
    }
}

impl WireDecode for DecodedOdrMemberDiscriminator {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Singleton)
            }
            2 => decode_id_variant(decoder, fields, Self::CallableApplication),
            3 => decode_id_variant(decoder, fields, Self::GeneratedCallable),
            4 => decode_id_variant(decoder, fields, Self::GeneratedNominal),
            5 => decode_id_variant(decoder, fields, Self::ExactType),
            6 => decode_id_variant(decoder, fields, Self::Layout),
            7 => decode_id_variant(decoder, fields, Self::Scan),
            8 => decode_id_variant(decoder, fields, Self::DispatchTable),
            9 => decode_id_variant(decoder, fields, Self::DispatchSlot),
            10 => decode_id_variant(decoder, fields, Self::StaticStorage),
            11 => decode_id_variant(decoder, fields, Self::ImmortalObject),
            12 => decode_id_variant(decoder, fields, Self::InitializationUnit),
            13 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, StructuralDefinitionPath::decode)
                    .map(Self::StructuralPath)
            }
            14 => decode_id_variant(decoder, fields, Self::CallableBody),
            15 => decode_id_variant(decoder, fields, Self::SafepointSite),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedOdrMemberKey {
    group: DecodedPersistentId<OdrGroupId>,
    role: OdrMemberRole,
    discriminator: DecodedOdrMemberDiscriminator,
}

impl DecodedOdrMemberKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<OdrMemberKey, OdrIdentityResolutionError<E>>
    where
        R: OdrMemberResolver<E>,
    {
        let group = resolver
            .resolve(self.group)
            .map_err(OdrIdentityResolutionError::Reference)?;
        let discriminator = self
            .discriminator
            .resolve(resolver)
            .map_err(OdrIdentityResolutionError::Reference)?;
        OdrMemberKey::new(group, self.role, discriminator)
            .map_err(OdrIdentityResolutionError::Member)
    }
}

impl WireEncode for DecodedOdrMemberKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.group.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.discriminator.encode(encoder)
    }
}

impl WireDecode for DecodedOdrMemberKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            group: decoder.field(1, DecodedPersistentId::decode)?,
            role: decoder.field(2, OdrMemberRole::decode)?,
            discriminator: decoder.field(3, DecodedOdrMemberDiscriminator::decode)?,
        })
    }
}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_>,
    fields: u64,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, DecodedPersistentId::decode).map(build)
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
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

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}
