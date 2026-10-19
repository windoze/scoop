use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, DecodedCallableTemplateOrigin, DecodedOptionalSignatureType,
    DecodedPersistentId, DecodedSignatureTypeKey, OptionalSignatureType,
    PersistentGeneratedCallableId, PersistentIdResolver, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{CallableDeclarationIdResolver, SignatureTypeReferenceResolver};

mod resources;

mod declarations;
pub use declarations::*;

/// One declaration-bound callable application in a default template.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultCallableRefV1 {
    declaration: DefaultCallableDeclarationV1,
    owner: OptionalSignatureType,
    type_arguments: Vec<SignatureTypeKey>,
    requirements: Vec<crate::DefaultRequirementBindingV1>,
}

impl DefaultCallableRefV1 {
    pub fn try_new(
        declaration: DefaultCallableDeclarationV1,
        owner: OptionalSignatureType,
        type_arguments: Vec<SignatureTypeKey>,
    ) -> Result<Self, DefaultCallableRefBuildError> {
        u32::try_from(type_arguments.len())
            .map_err(|_| DefaultCallableRefBuildError::TooManyTypeArguments)?;
        Ok(Self {
            declaration,
            owner,
            type_arguments,
            requirements: Vec::new(),
        })
    }

    pub fn with_requirements(
        mut self,
        requirements: Vec<crate::DefaultRequirementBindingV1>,
    ) -> Self {
        self.requirements = requirements;
        self
    }
    pub fn requirements(&self) -> &[crate::DefaultRequirementBindingV1] {
        &self.requirements
    }

    pub const fn declaration(&self) -> DefaultCallableDeclarationV1 {
        self.declaration
    }

    pub const fn owner(&self) -> &OptionalSignatureType {
        &self.owner
    }

    pub fn type_arguments(&self) -> &[SignatureTypeKey] {
        &self.type_arguments
    }
}

impl WireEncode for DefaultCallableRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if self.requirements.is_empty() { 3 } else { 4 })?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.type_arguments)?;
        if !self.requirements.is_empty() {
            encoder.field(4)?;
            encode_sequence(encoder, &self.requirements)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultCallableRefV1 {
    declaration: DecodedDefaultCallableDeclarationV1,
    owner: DecodedOptionalSignatureType,
    type_arguments: Vec<DecodedSignatureTypeKey>,
    requirements: Vec<crate::DecodedDefaultRequirementBindingV1>,
}

impl DecodedDefaultCallableRefV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultCallableRefV1, DefaultCallableRefResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(DefaultCallableRefResolutionError::Declaration)?;
        let owner = self
            .owner
            .resolve(resolver)
            .map_err(DefaultCallableRefResolutionError::Owner)?;
        let count = u32::try_from(self.type_arguments.len())
            .map_err(|_| DefaultCallableRefResolutionError::TooManyTypeArguments)?;
        let mut type_arguments = Vec::with_capacity(count as usize);
        for (index, argument) in self.type_arguments.into_iter().enumerate() {
            type_arguments.push(argument.resolve(resolver).map_err(|error| {
                DefaultCallableRefResolutionError::TypeArgument { index, error }
            })?);
        }
        let requirements = self
            .requirements
            .into_iter()
            .map(|binding| {
                binding.resolve(resolver).map_err(|error| {
                    DefaultCallableRefResolutionError::Requirement(Box::new(error))
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(DefaultCallableRefV1 {
            declaration,
            owner,
            type_arguments,
            requirements,
        })
    }
}

impl WireEncode for DecodedDefaultCallableRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if self.requirements.is_empty() { 3 } else { 4 })?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.type_arguments)?;
        if !self.requirements.is_empty() {
            encoder.field(4)?;
            encode_sequence(encoder, &self.requirements)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedDefaultCallableRefV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields != 3 && fields != 4 {
            return Err(wire_error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected: 3,
                    actual: fields,
                },
            ));
        }
        Ok(Self {
            declaration: decoder.field(1, DecodedDefaultCallableDeclarationV1::decode)?,
            owner: decoder.field(2, DecodedOptionalSignatureType::decode)?,
            type_arguments: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedSignatureTypeKey::decode(decoder))
            })?,
            requirements: if fields == 4 {
                decoder.field(4, |d| {
                    d.decode_array(|d, _| crate::DecodedDefaultRequirementBindingV1::decode(d))
                })?
            } else {
                Vec::new()
            },
        })
    }
}

mod bound;
pub use bound::*;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultMethodCalleeV1 {
    Callable(DefaultCallableRefV1),
    Bound(Box<DefaultBoundCallableRefV1>),
    DerivedEquality { owner_type: SignatureTypeKey },
}

impl WireEncode for DefaultMethodCalleeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 1,
            Self::Bound(_) => 2,
            Self::DerivedEquality { .. } => 3,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(callable) => callable.encode(encoder),
            Self::Bound(bound) => bound.encode(encoder),
            Self::DerivedEquality { owner_type } => owner_type.encode(encoder),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultMethodCalleeV1 {
    Callable(DecodedDefaultCallableRefV1),
    Bound(Box<DecodedDefaultBoundCallableRefV1>),
    DerivedEquality { owner_type: DecodedSignatureTypeKey },
}

impl DecodedDefaultMethodCalleeV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultMethodCalleeV1, DefaultMethodCalleeResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        match self {
            Self::Callable(callable) => callable
                .resolve(resolver)
                .map(DefaultMethodCalleeV1::Callable)
                .map_err(DefaultMethodCalleeResolutionError::Callable),
            Self::Bound(bound) => bound
                .resolve(resolver)
                .map(|bound| DefaultMethodCalleeV1::Bound(Box::new(bound)))
                .map_err(DefaultMethodCalleeResolutionError::Bound),
            Self::DerivedEquality { owner_type } => owner_type
                .resolve(resolver)
                .map(|owner_type| DefaultMethodCalleeV1::DerivedEquality { owner_type })
                .map_err(DefaultMethodCalleeResolutionError::DerivedEqualityOwner),
        }
    }
}

impl WireEncode for DecodedDefaultMethodCalleeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 1,
            Self::Bound(_) => 2,
            Self::DerivedEquality { .. } => 3,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(callable) => callable.encode(encoder),
            Self::Bound(bound) => bound.encode(encoder),
            Self::DerivedEquality { owner_type } => owner_type.encode(encoder),
        }
    }
}

impl WireDecode for DecodedDefaultMethodCalleeV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedDefaultCallableRefV1::decode)
                .map(Self::Callable),
            2 => decoder
                .field(1, DecodedDefaultBoundCallableRefV1::decode)
                .map(|bound| Self::Bound(Box::new(bound))),
            3 => decoder
                .field(1, DecodedSignatureTypeKey::decode)
                .map(|owner_type| Self::DerivedEquality { owner_type }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

pub trait DefaultCallableReferenceResolver<E>:
    SignatureTypeReferenceResolver<E>
    + CallableDeclarationIdResolver<E>
    + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
    + crate::TemplateLocalReferenceResolver<E>
    + crate::DefaultConstructorReferenceResolver<E>
    + crate::DefaultFieldReferenceResolver<E>
    + PersistentIdResolver<scoop_identity::PersistentExactTypeId, Error = E>
    + PersistentIdResolver<scoop_identity::PersistentAnnotationId, Error = E>
    + PersistentIdResolver<scoop_identity::PersistentPropertyId, Error = E>
    + PersistentIdResolver<scoop_identity::PersistentExtensionPropertyId, Error = E>
    + PersistentIdResolver<scoop_identity::PersistentObjectValueId, Error = E>
    + PersistentIdResolver<scoop_identity::PersistentCallbackRegistrationId, Error = E>
    + PersistentIdResolver<scoop_identity::PersistentInitializationUnitId, Error = E>
{
}

impl<R, E> DefaultCallableReferenceResolver<E> for R where
    R: SignatureTypeReferenceResolver<E>
        + CallableDeclarationIdResolver<E>
        + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
        + crate::TemplateLocalReferenceResolver<E>
        + crate::DefaultConstructorReferenceResolver<E>
        + crate::DefaultFieldReferenceResolver<E>
        + PersistentIdResolver<scoop_identity::PersistentExactTypeId, Error = E>
        + PersistentIdResolver<scoop_identity::PersistentAnnotationId, Error = E>
        + PersistentIdResolver<scoop_identity::PersistentPropertyId, Error = E>
        + PersistentIdResolver<scoop_identity::PersistentExtensionPropertyId, Error = E>
        + PersistentIdResolver<scoop_identity::PersistentObjectValueId, Error = E>
        + PersistentIdResolver<scoop_identity::PersistentCallbackRegistrationId, Error = E>
        + PersistentIdResolver<scoop_identity::PersistentInitializationUnitId, Error = E>
{
}

mod errors;
pub use errors::*;

fn encode_sequence(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests;
