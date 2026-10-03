use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    AccessorRole, CallableApplicationKey, CallableArguments, CallableInstantiationOwner,
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOrigin,
    CallableTemplateOwner, PropertyAccessorKey,
};
use crate::{
    DecodedPersistentId, DecodedPropertyOwner, NonEmptyVec, PersistentCallableApplicationId,
    PersistentConstructorId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentExtensionPropertyId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentIdResolver, PersistentInitializationUnitId,
    PersistentPropertyAccessorId, PersistentPropertyId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedPropertyAccessorKey {
    owner: DecodedPropertyOwner,
    role: AccessorRole,
}

impl DecodedPropertyAccessorKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<PropertyAccessorKey, E>
    where
        R: PersistentIdResolver<PersistentPropertyId, Error = E>
            + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>,
    {
        self.owner
            .resolve(resolver)
            .map(|owner| PropertyAccessorKey::new(owner, self.role))
    }
}

impl WireEncode for DecodedPropertyAccessorKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl WireDecode for DecodedPropertyAccessorKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            owner: decoder.field(1, DecodedPropertyOwner::decode)?,
            role: decoder.field(2, decode_accessor_role)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCallableTemplateOrigin {
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    Accessor(DecodedPersistentId<PersistentPropertyAccessorId>),
    VariantConstructor(DecodedPersistentId<PersistentEnumVariantId>),
}

impl DecodedCallableTemplateOrigin {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CallableTemplateOrigin, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<PersistentEnumVariantId, Error = E>,
    {
        match self {
            Self::Function(id) => resolver.resolve(id).map(CallableTemplateOrigin::Function),
            Self::GenericFunction(id) => resolver
                .resolve(id)
                .map(CallableTemplateOrigin::GenericFunction),
            Self::Constructor(id) => resolver
                .resolve(id)
                .map(CallableTemplateOrigin::Constructor),
            Self::Accessor(id) => resolver.resolve(id).map(CallableTemplateOrigin::Accessor),
            Self::VariantConstructor(id) => resolver
                .resolve(id)
                .map(CallableTemplateOrigin::VariantConstructor),
        }
    }
}

impl WireEncode for DecodedCallableTemplateOrigin {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 2, id),
            Self::Constructor(id) => encode_value_sum(encoder, 3, id),
            Self::Accessor(id) => encode_value_sum(encoder, 4, id),
            Self::VariantConstructor(id) => encode_value_sum(encoder, 5, id),
        }
    }
}

impl WireDecode for DecodedCallableTemplateOrigin {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericFunction),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Accessor),
            5 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::VariantConstructor),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCallableInstantiationOwner {
    NoOwner,
    ExactNominalOwner(DecodedPersistentId<PersistentExactTypeId>),
    EnclosingCallableApplication(DecodedPersistentId<PersistentCallableApplicationId>),
    EnclosingInitializationApplication(DecodedPersistentId<PersistentInitializationUnitId>),
}

impl DecodedCallableInstantiationOwner {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CallableInstantiationOwner, E>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>,
    {
        match self {
            Self::NoOwner => Ok(CallableInstantiationOwner::NoOwner),
            Self::ExactNominalOwner(id) => resolver
                .resolve(id)
                .map(CallableInstantiationOwner::ExactNominalOwner),
            Self::EnclosingCallableApplication(id) => resolver
                .resolve(id)
                .map(CallableInstantiationOwner::EnclosingCallableApplication),
            Self::EnclosingInitializationApplication(id) => resolver
                .resolve(id)
                .map(CallableInstantiationOwner::EnclosingInitializationApplication),
        }
    }
}

impl WireEncode for DecodedCallableInstantiationOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoOwner => encode_empty_sum(encoder, 1),
            Self::ExactNominalOwner(id) => encode_value_sum(encoder, 2, id),
            Self::EnclosingCallableApplication(id) => encode_value_sum(encoder, 3, id),
            Self::EnclosingInitializationApplication(id) => encode_value_sum(encoder, 4, id),
        }
    }
}

impl WireDecode for DecodedCallableInstantiationOwner {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::NoOwner)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::ExactNominalOwner)
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::EnclosingCallableApplication)
            }
            4 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::EnclosingInitializationApplication)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedCallableArguments {
    NoCallableArguments,
    Arguments(NonEmptyVec<DecodedPersistentId<PersistentExactTypeId>>),
}

impl WireEncode for DecodedCallableArguments {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoCallableArguments => encode_empty_sum(encoder, 1),
            Self::Arguments(arguments) => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                encoder.array(arguments.as_slice().len() as u64)?;
                for argument in arguments.as_slice() {
                    argument.encode(encoder)?;
                }
                Ok(())
            }
        }
    }
}

impl WireDecode for DecodedCallableArguments {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::NoCallableArguments)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                let arguments = decoder.field(1, |decoder| {
                    decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
                })?;
                NonEmptyVec::new(arguments)
                    .map(Self::Arguments)
                    .map_err(|_| invalid_empty_sequence(decoder))
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCallableApplicationKey {
    origin: DecodedCallableTemplateOrigin,
    instantiation_owner: DecodedCallableInstantiationOwner,
    callable_arguments: DecodedCallableArguments,
}

impl DecodedCallableApplicationKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CallableApplicationKey, CallableApplicationResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<PersistentEnumVariantId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>,
    {
        let origin = self
            .origin
            .resolve(resolver)
            .map_err(CallableApplicationResolutionError::Reference)?;
        let owner = self
            .instantiation_owner
            .resolve(resolver)
            .map_err(CallableApplicationResolutionError::Reference)?;
        let arguments = resolve_arguments(self.callable_arguments, resolver)?;
        rebuild_application(origin, owner, arguments)
    }
}

impl WireEncode for DecodedCallableApplicationKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.instantiation_owner.encode(encoder)?;
        encoder.field(3)?;
        self.callable_arguments.encode(encoder)
    }
}

impl WireDecode for DecodedCallableApplicationKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            origin: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            instantiation_owner: decoder.field(2, DecodedCallableInstantiationOwner::decode)?,
            callable_arguments: decoder.field(3, DecodedCallableArguments::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableApplicationResolutionError<E> {
    Reference(E),
    Allocation,
    Shape,
}

impl<E: fmt::Display> fmt::Display for CallableApplicationResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Allocation => formatter.write_str("failed to allocate callable arguments"),
            Self::Shape => formatter.write_str(
                "callable template kind does not match the presence of callable arguments",
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CallableApplicationResolutionError<E> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCallableTemplateOwner {
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    Accessor(DecodedPersistentId<PersistentPropertyAccessorId>),
    Generated(DecodedPersistentId<PersistentGeneratedCallableId>),
    VariantConstructor(DecodedPersistentId<PersistentEnumVariantId>),
}

impl DecodedCallableTemplateOwner {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CallableTemplateOwner, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentIdResolver<PersistentEnumVariantId, Error = E>,
    {
        match self {
            Self::Function(id) => resolver.resolve(id).map(CallableTemplateOwner::Function),
            Self::GenericFunction(id) => resolver
                .resolve(id)
                .map(CallableTemplateOwner::GenericFunction),
            Self::Constructor(id) => resolver.resolve(id).map(CallableTemplateOwner::Constructor),
            Self::Accessor(id) => resolver.resolve(id).map(CallableTemplateOwner::Accessor),
            Self::Generated(id) => resolver.resolve(id).map(CallableTemplateOwner::Generated),
            Self::VariantConstructor(id) => resolver
                .resolve(id)
                .map(CallableTemplateOwner::VariantConstructor),
        }
    }
}

impl WireEncode for DecodedCallableTemplateOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 2, id),
            Self::Constructor(id) => encode_value_sum(encoder, 3, id),
            Self::Accessor(id) => encode_value_sum(encoder, 4, id),
            Self::Generated(id) => encode_value_sum(encoder, 5, id),
            Self::VariantConstructor(id) => encode_value_sum(encoder, 6, id),
        }
    }
}

impl WireDecode for DecodedCallableTemplateOwner {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericFunction),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Accessor),
            5 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Generated),
            6 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::VariantConstructor),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCallableMaterializationContext {
    NoSubstitution,
    Application(DecodedPersistentId<PersistentCallableApplicationId>),
    InitializationApplication(DecodedPersistentId<PersistentInitializationUnitId>),
}

impl DecodedCallableMaterializationContext {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CallableMaterializationContext, E>
    where
        R: PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>,
    {
        match self {
            Self::NoSubstitution => Ok(CallableMaterializationContext::NoSubstitution),
            Self::Application(id) => resolver
                .resolve(id)
                .map(CallableMaterializationContext::Application),
            Self::InitializationApplication(id) => resolver
                .resolve(id)
                .map(CallableMaterializationContext::InitializationApplication),
        }
    }
}

impl WireEncode for DecodedCallableMaterializationContext {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoSubstitution => encode_empty_sum(encoder, 1),
            Self::Application(id) => encode_value_sum(encoder, 2, id),
            Self::InitializationApplication(id) => encode_value_sum(encoder, 3, id),
        }
    }
}

impl WireDecode for DecodedCallableMaterializationContext {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::NoSubstitution)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Application)
            }
            3 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::InitializationApplication)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedCallableMaterialization {
    template: DecodedCallableTemplateOwner,
    context: DecodedCallableMaterializationContext,
}

impl DecodedCallableMaterialization {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CallableMaterialization, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentIdResolver<PersistentEnumVariantId, Error = E>
            + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentInitializationUnitId, Error = E>,
    {
        Ok(CallableMaterialization::new(
            self.template.resolve(resolver)?,
            self.context.resolve(resolver)?,
        ))
    }
}

impl WireEncode for DecodedCallableMaterialization {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.template.encode(encoder)?;
        encoder.field(2)?;
        self.context.encode(encoder)
    }
}

impl WireDecode for DecodedCallableMaterialization {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            template: decoder.field(1, DecodedCallableTemplateOwner::decode)?,
            context: decoder.field(2, DecodedCallableMaterializationContext::decode)?,
        })
    }
}

fn resolve_arguments<R, E>(
    arguments: DecodedCallableArguments,
    resolver: &mut R,
) -> Result<CallableArguments, CallableApplicationResolutionError<E>>
where
    R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
{
    match arguments {
        DecodedCallableArguments::NoCallableArguments => Ok(CallableArguments::NoCallableArguments),
        DecodedCallableArguments::Arguments(arguments) => {
            let mut resolved = Vec::new();
            resolved
                .try_reserve_exact(arguments.as_slice().len())
                .map_err(|_| CallableApplicationResolutionError::Allocation)?;
            for argument in arguments.as_slice() {
                resolved.push(
                    resolver
                        .resolve(*argument)
                        .map_err(CallableApplicationResolutionError::Reference)?,
                );
            }
            NonEmptyVec::new(resolved)
                .map(CallableArguments::Arguments)
                .map_err(|_| CallableApplicationResolutionError::Shape)
        }
    }
}

fn rebuild_application<E>(
    origin: CallableTemplateOrigin,
    owner: CallableInstantiationOwner,
    arguments: CallableArguments,
) -> Result<CallableApplicationKey, CallableApplicationResolutionError<E>> {
    match (origin, arguments) {
        (CallableTemplateOrigin::Function(origin), CallableArguments::NoCallableArguments) => {
            Ok(CallableApplicationKey::for_function(origin, owner))
        }
        (
            CallableTemplateOrigin::GenericFunction(origin),
            CallableArguments::Arguments(arguments),
        ) => Ok(CallableApplicationKey::for_generic_function(
            origin, owner, arguments,
        )),
        (CallableTemplateOrigin::Constructor(origin), CallableArguments::NoCallableArguments) => {
            Ok(CallableApplicationKey::for_constructor(origin, owner))
        }
        (CallableTemplateOrigin::Accessor(origin), CallableArguments::NoCallableArguments) => {
            Ok(CallableApplicationKey::for_accessor(origin, owner))
        }
        (CallableTemplateOrigin::Accessor(origin), CallableArguments::Arguments(arguments)) => Ok(
            CallableApplicationKey::for_generic_extension_accessor(origin, owner, arguments),
        ),
        (
            CallableTemplateOrigin::VariantConstructor(origin),
            CallableArguments::NoCallableArguments,
        ) => Ok(CallableApplicationKey::for_variant_constructor(
            origin, owner,
        )),
        _ => Err(CallableApplicationResolutionError::Shape),
    }
}

fn decode_accessor_role(decoder: &mut Decoder<'_>) -> Result<AccessorRole, WireError> {
    match decoder.unsigned()? {
        1 => Ok(AccessorRole::Getter),
        2 => Ok(AccessorRole::Setter),
        tag => Err(unknown_tag(decoder, tag)),
    }
}

fn decode_sum_header(decoder: &mut Decoder<'_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
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

fn invalid_empty_sequence(decoder: &Decoder<'_>) -> WireError {
    WireError::new(
        WireErrorKind::InvalidLength {
            expected: 1,
            actual: 0,
        },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
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

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
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
mod tests;
