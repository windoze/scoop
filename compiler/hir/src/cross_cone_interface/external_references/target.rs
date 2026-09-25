use std::fmt;

use scoop_identity::{
    BindableEntity, CallableTemplateOrigin, DecodedCallableTemplateOrigin,
    DecodedNominalDeclarationOwner, DecodedPersistentId, DecodedPropertyOwner,
    NominalDeclarationOwner, PersistentConstructorId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExtensionPropertyId, PersistentFieldId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentIdResolver, PersistentObjectValueId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId,
    PropertyOwner,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Kind-preserving semantic identity of one foreign entity referenced by a
/// cross-Cone HIR interface.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExternalHirTargetV1 {
    Nominal(NominalDeclarationOwner),
    Callable(CallableTemplateOrigin),
    Property(PropertyOwner),
    ObjectValue(PersistentObjectValueId),
    TypeAlias(PersistentTypeAliasId),
    Field(PersistentFieldId),
    EnumVariantField(PersistentEnumVariantFieldId),
    GeneratedCallable(PersistentGeneratedCallableId),
}

impl From<NominalDeclarationOwner> for ExternalHirTargetV1 {
    fn from(target: NominalDeclarationOwner) -> Self {
        Self::Nominal(target)
    }
}

impl From<BindableEntity> for ExternalHirTargetV1 {
    fn from(target: BindableEntity) -> Self {
        match target {
            BindableEntity::Type(target) => {
                Self::Nominal(NominalDeclarationOwner::Concrete(target))
            }
            BindableEntity::GenericType(target) => {
                Self::Nominal(NominalDeclarationOwner::GenericTemplate(target))
            }
            BindableEntity::ObjectValue(target) => Self::ObjectValue(target),
            BindableEntity::Function(target) => {
                Self::Callable(CallableTemplateOrigin::Function(target))
            }
            BindableEntity::GenericFunction(target) => {
                Self::Callable(CallableTemplateOrigin::GenericFunction(target))
            }
            BindableEntity::Property(target) => Self::Property(PropertyOwner::Property(target)),
            BindableEntity::ExtensionProperty(target) => {
                Self::Property(PropertyOwner::ExtensionProperty(target))
            }
            BindableEntity::TypeAlias(target) => Self::TypeAlias(target),
            BindableEntity::EnumVariant(target) => {
                Self::Callable(CallableTemplateOrigin::VariantConstructor(target))
            }
        }
    }
}

impl WireEncode for ExternalHirTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Nominal(_) => 1,
            Self::Callable(_) => 2,
            Self::Property(_) => 3,
            Self::ObjectValue(_) => 4,
            Self::TypeAlias(_) => 5,
            Self::Field(_) => 6,
            Self::EnumVariantField(_) => 7,
            Self::GeneratedCallable(_) => 8,
        })?;
        encoder.field(1)?;
        match self {
            Self::Nominal(target) => target.encode(encoder),
            Self::Callable(target) => target.encode(encoder),
            Self::Property(target) => target.encode(encoder),
            Self::ObjectValue(target) => target.encode(encoder),
            Self::TypeAlias(target) => target.encode(encoder),
            Self::Field(target) => target.encode(encoder),
            Self::EnumVariantField(target) => target.encode(encoder),
            Self::GeneratedCallable(target) => target.encode(encoder),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedExternalHirTargetV1 {
    Nominal(DecodedNominalDeclarationOwner),
    Callable(DecodedCallableTemplateOrigin),
    Property(DecodedPropertyOwner),
    ObjectValue(DecodedPersistentId<PersistentObjectValueId>),
    TypeAlias(DecodedPersistentId<PersistentTypeAliasId>),
    Field(DecodedPersistentId<PersistentFieldId>),
    EnumVariantField(DecodedPersistentId<PersistentEnumVariantFieldId>),
    GeneratedCallable(DecodedPersistentId<PersistentGeneratedCallableId>),
}

impl DecodedExternalHirTargetV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExternalHirTargetV1, ExternalHirTargetResolutionError<E>>
    where
        R: ExternalHirTargetResolver<E>,
    {
        match self {
            Self::Nominal(target) => target
                .resolve(resolver)
                .map(ExternalHirTargetV1::Nominal)
                .map_err(ExternalHirTargetResolutionError::Nominal),
            Self::Callable(target) => target
                .resolve(resolver)
                .map(ExternalHirTargetV1::Callable)
                .map_err(ExternalHirTargetResolutionError::Callable),
            Self::Property(target) => target
                .resolve(resolver)
                .map(ExternalHirTargetV1::Property)
                .map_err(ExternalHirTargetResolutionError::Property),
            Self::ObjectValue(target) => resolver
                .resolve(target)
                .map(ExternalHirTargetV1::ObjectValue)
                .map_err(ExternalHirTargetResolutionError::ObjectValue),
            Self::TypeAlias(target) => resolver
                .resolve(target)
                .map(ExternalHirTargetV1::TypeAlias)
                .map_err(ExternalHirTargetResolutionError::TypeAlias),
            Self::Field(target) => resolver
                .resolve(target)
                .map(ExternalHirTargetV1::Field)
                .map_err(ExternalHirTargetResolutionError::Field),
            Self::EnumVariantField(target) => resolver
                .resolve(target)
                .map(ExternalHirTargetV1::EnumVariantField)
                .map_err(ExternalHirTargetResolutionError::EnumVariantField),
            Self::GeneratedCallable(target) => resolver
                .resolve(target)
                .map(ExternalHirTargetV1::GeneratedCallable)
                .map_err(ExternalHirTargetResolutionError::GeneratedCallable),
        }
    }
}

impl WireEncode for DecodedExternalHirTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Nominal(_) => 1,
            Self::Callable(_) => 2,
            Self::Property(_) => 3,
            Self::ObjectValue(_) => 4,
            Self::TypeAlias(_) => 5,
            Self::Field(_) => 6,
            Self::EnumVariantField(_) => 7,
            Self::GeneratedCallable(_) => 8,
        })?;
        encoder.field(1)?;
        match self {
            Self::Nominal(target) => target.encode(encoder),
            Self::Callable(target) => target.encode(encoder),
            Self::Property(target) => target.encode(encoder),
            Self::ObjectValue(target) => target.encode(encoder),
            Self::TypeAlias(target) => target.encode(encoder),
            Self::Field(target) => target.encode(encoder),
            Self::EnumVariantField(target) => target.encode(encoder),
            Self::GeneratedCallable(target) => target.encode(encoder),
        }
    }
}

impl WireDecode for DecodedExternalHirTargetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => decoder
                .field(1, DecodedNominalDeclarationOwner::decode)
                .map(Self::Nominal),
            2 => decoder
                .field(1, DecodedCallableTemplateOrigin::decode)
                .map(Self::Callable),
            3 => decoder
                .field(1, DecodedPropertyOwner::decode)
                .map(Self::Property),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::ObjectValue),
            5 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::TypeAlias),
            6 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Field),
            7 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::EnumVariantField),
            8 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GeneratedCallable),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

pub trait ExternalHirTargetResolver<E>:
    PersistentIdResolver<PersistentTypeId, Error = E>
    + PersistentIdResolver<PersistentGenericTypeId, Error = E>
    + PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
    + PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
    + PersistentIdResolver<PersistentPropertyId, Error = E>
    + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
    + PersistentIdResolver<PersistentObjectValueId, Error = E>
    + PersistentIdResolver<PersistentTypeAliasId, Error = E>
    + PersistentIdResolver<PersistentFieldId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantFieldId, Error = E>
    + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
{
}

impl<R, E> ExternalHirTargetResolver<E> for R where
    R: PersistentIdResolver<PersistentTypeId, Error = E>
        + PersistentIdResolver<PersistentGenericTypeId, Error = E>
        + PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
        + PersistentIdResolver<PersistentConstructorId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
        + PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
        + PersistentIdResolver<PersistentObjectValueId, Error = E>
        + PersistentIdResolver<PersistentTypeAliasId, Error = E>
        + PersistentIdResolver<PersistentFieldId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantFieldId, Error = E>
        + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExternalHirTargetResolutionError<E> {
    Nominal(E),
    Callable(E),
    Property(E),
    ObjectValue(E),
    TypeAlias(E),
    Field(E),
    EnumVariantField(E),
    GeneratedCallable(E),
}

impl<E: fmt::Display> fmt::Display for ExternalHirTargetResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, error) = match self {
            Self::Nominal(error) => ("nominal", error),
            Self::Callable(error) => ("callable", error),
            Self::Property(error) => ("property", error),
            Self::ObjectValue(error) => ("object value", error),
            Self::TypeAlias(error) => ("type alias", error),
            Self::Field(error) => ("field", error),
            Self::EnumVariantField(error) => ("enum variant field", error),
            Self::GeneratedCallable(error) => ("generated callable", error),
        };
        write!(formatter, "invalid external HIR {kind} target: {error}")
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExternalHirTargetResolutionError<E> {}

#[cfg(test)]
mod tests;
