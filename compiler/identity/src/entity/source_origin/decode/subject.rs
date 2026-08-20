use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{DecodedDefinitionOrigin, SourceOriginResolutionError};
use crate::{
    ConeIdentity, DecodedPersistentId, DefinitionOriginRecord, DefinitionOriginSubject,
    PersistentCallbackRegistrationId, PersistentConstructorId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExtensionPropertyId, PersistentFieldId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentIdResolver, PersistentInitializationUnitId,
    PersistentKeyResolver, PersistentLocalBindingId, PersistentLocalValueId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentSourceContextId,
    PersistentSourceNativeExternalContractId, PersistentTypeAliasId, PersistentTypeId,
    SourceContextKey,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedDefinitionOriginSubject {
    Type(DecodedPersistentId<PersistentTypeId>),
    GenericType(DecodedPersistentId<PersistentGenericTypeId>),
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    Property(DecodedPersistentId<PersistentPropertyId>),
    ExtensionProperty(DecodedPersistentId<PersistentExtensionPropertyId>),
    PropertyAccessor(DecodedPersistentId<PersistentPropertyAccessorId>),
    TypeAlias(DecodedPersistentId<PersistentTypeAliasId>),
    Field(DecodedPersistentId<PersistentFieldId>),
    EnumVariant(DecodedPersistentId<PersistentEnumVariantId>),
    EnumVariantField(DecodedPersistentId<PersistentEnumVariantFieldId>),
    GeneratedCallable(DecodedPersistentId<PersistentGeneratedCallableId>),
    InitializationUnit(DecodedPersistentId<PersistentInitializationUnitId>),
    LocalBinding(DecodedPersistentId<PersistentLocalBindingId>),
    LocalValue(DecodedPersistentId<PersistentLocalValueId>),
    CallbackRegistration(DecodedPersistentId<PersistentCallbackRegistrationId>),
    SourceNativeContract(DecodedPersistentId<PersistentSourceNativeExternalContractId>),
}

impl DecodedDefinitionOriginSubject {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DefinitionOriginSubject, E>
    where
        R: DefinitionOriginSubjectResolver<E>,
    {
        match self {
            Self::Type(id) => resolver.resolve(id).map(DefinitionOriginSubject::Type),
            Self::GenericType(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::GenericType),
            Self::Function(id) => resolver.resolve(id).map(DefinitionOriginSubject::Function),
            Self::GenericFunction(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::GenericFunction),
            Self::Constructor(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::Constructor),
            Self::Property(id) => resolver.resolve(id).map(DefinitionOriginSubject::Property),
            Self::ExtensionProperty(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::ExtensionProperty),
            Self::PropertyAccessor(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::PropertyAccessor),
            Self::TypeAlias(id) => resolver.resolve(id).map(DefinitionOriginSubject::TypeAlias),
            Self::Field(id) => resolver.resolve(id).map(DefinitionOriginSubject::Field),
            Self::EnumVariant(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::EnumVariant),
            Self::EnumVariantField(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::EnumVariantField),
            Self::GeneratedCallable(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::GeneratedCallable),
            Self::InitializationUnit(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::InitializationUnit),
            Self::LocalBinding(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::LocalBinding),
            Self::LocalValue(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::LocalValue),
            Self::CallbackRegistration(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::CallbackRegistration),
            Self::SourceNativeContract(id) => resolver
                .resolve(id)
                .map(DefinitionOriginSubject::SourceNativeContract),
        }
    }
}

impl WireEncode for DecodedDefinitionOriginSubject {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, id): (u64, &dyn WireEncode) = match self {
            Self::Type(id) => (1, id),
            Self::GenericType(id) => (2, id),
            Self::Function(id) => (3, id),
            Self::GenericFunction(id) => (4, id),
            Self::Constructor(id) => (5, id),
            Self::Property(id) => (6, id),
            Self::ExtensionProperty(id) => (7, id),
            Self::PropertyAccessor(id) => (8, id),
            Self::TypeAlias(id) => (9, id),
            Self::Field(id) => (10, id),
            Self::EnumVariant(id) => (11, id),
            Self::EnumVariantField(id) => (12, id),
            Self::GeneratedCallable(id) => (13, id),
            Self::InitializationUnit(id) => (14, id),
            Self::LocalBinding(id) => (15, id),
            Self::LocalValue(id) => (16, id),
            Self::CallbackRegistration(id) => (17, id),
            Self::SourceNativeContract(id) => (18, id),
        };
        encode_value_sum(encoder, tag, id)
    }
}

impl WireDecode for DecodedDefinitionOriginSubject {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decode_id_variant(decoder, Self::Type),
            2 => decode_id_variant(decoder, Self::GenericType),
            3 => decode_id_variant(decoder, Self::Function),
            4 => decode_id_variant(decoder, Self::GenericFunction),
            5 => decode_id_variant(decoder, Self::Constructor),
            6 => decode_id_variant(decoder, Self::Property),
            7 => decode_id_variant(decoder, Self::ExtensionProperty),
            8 => decode_id_variant(decoder, Self::PropertyAccessor),
            9 => decode_id_variant(decoder, Self::TypeAlias),
            10 => decode_id_variant(decoder, Self::Field),
            11 => decode_id_variant(decoder, Self::EnumVariant),
            12 => decode_id_variant(decoder, Self::EnumVariantField),
            13 => decode_id_variant(decoder, Self::GeneratedCallable),
            14 => decode_id_variant(decoder, Self::InitializationUnit),
            15 => decode_id_variant(decoder, Self::LocalBinding),
            16 => decode_id_variant(decoder, Self::LocalValue),
            17 => decode_id_variant(decoder, Self::CallbackRegistration),
            18 => decode_id_variant(decoder, Self::SourceNativeContract),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefinitionOriginRecord {
    subject: DecodedDefinitionOriginSubject,
    origin: DecodedDefinitionOrigin,
}

impl DecodedDefinitionOriginRecord {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefinitionOriginRecord, DefinitionOriginRecordResolutionError<E>>
    where
        R: DefinitionOriginSubjectResolver<E>
            + PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>,
    {
        let subject = self
            .subject
            .resolve(resolver)
            .map_err(DefinitionOriginRecordResolutionError::Subject)?;
        let origin = self
            .origin
            .resolve(resolver)
            .map_err(DefinitionOriginRecordResolutionError::Origin)?;
        Ok(DefinitionOriginRecord::new(subject, origin))
    }
}

impl WireEncode for DecodedDefinitionOriginRecord {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.subject.encode(encoder)?;
        encoder.field(2)?;
        self.origin.encode(encoder)
    }
}

impl WireDecode for DecodedDefinitionOriginRecord {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            subject: decoder.field(1, DecodedDefinitionOriginSubject::decode)?,
            origin: decoder.field(2, DecodedDefinitionOrigin::decode)?,
        })
    }
}

pub trait DefinitionOriginSubjectResolver<E>:
    PersistentIdResolver<PersistentTypeId, Error = E>
    + PersistentIdResolver<PersistentGenericTypeId, Error = E>
    + PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
    + PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentPropertyId, Error = E>
    + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentIdResolver<PersistentTypeAliasId, Error = E>
    + PersistentIdResolver<PersistentFieldId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantFieldId, Error = E>
    + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
    + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
    + PersistentIdResolver<PersistentLocalBindingId, Error = E>
    + PersistentIdResolver<PersistentLocalValueId, Error = E>
    + PersistentIdResolver<PersistentCallbackRegistrationId, Error = E>
    + PersistentIdResolver<PersistentSourceNativeExternalContractId, Error = E>
{
}

impl<R, E> DefinitionOriginSubjectResolver<E> for R where
    R: PersistentIdResolver<PersistentTypeId, Error = E>
        + PersistentIdResolver<PersistentGenericTypeId, Error = E>
        + PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
        + PersistentIdResolver<PersistentConstructorId, Error = E>
        + PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentIdResolver<PersistentTypeAliasId, Error = E>
        + PersistentIdResolver<PersistentFieldId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantFieldId, Error = E>
        + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
        + PersistentIdResolver<PersistentInitializationUnitId, Error = E>
        + PersistentIdResolver<PersistentLocalBindingId, Error = E>
        + PersistentIdResolver<PersistentLocalValueId, Error = E>
        + PersistentIdResolver<PersistentCallbackRegistrationId, Error = E>
        + PersistentIdResolver<PersistentSourceNativeExternalContractId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinitionOriginRecordResolutionError<E> {
    Subject(E),
    Origin(SourceOriginResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for DefinitionOriginRecordResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Subject(error) => error.fmt(formatter),
            Self::Origin(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefinitionOriginRecordResolutionError<E>
{
}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_>,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
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

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &dyn WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}
