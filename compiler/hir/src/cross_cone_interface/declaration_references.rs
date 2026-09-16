use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, DecodedCallableTemplateOrigin, DecodedNominalDeclarationOwner,
    DecodedPropertyOwner, NominalDeclarationOwner, PersistentConstructorId,
    PersistentEnumVariantId, PersistentExtensionPropertyId, PersistentFunctionId,
    PersistentGenericFunctionId, PersistentIdResolver, PersistentPropertyAccessorId,
    PersistentPropertyId, PropertyOwner,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

pub type SourceNominalId = NominalDeclarationOwner;
pub type DecodedSourceNominalId = DecodedNominalDeclarationOwner;
pub type CallableDeclarationId = CallableTemplateOrigin;
pub type DecodedCallableDeclarationId = DecodedCallableTemplateOrigin;
pub type PropertyDeclarationId = PropertyOwner;
pub type DecodedPropertyDeclarationId = DecodedPropertyOwner;

pub trait SourceNominalIdResolver<E>:
    PersistentIdResolver<scoop_identity::PersistentTypeId, Error = E>
    + PersistentIdResolver<scoop_identity::PersistentGenericTypeId, Error = E>
{
}

impl<R, E> SourceNominalIdResolver<E> for R where
    R: PersistentIdResolver<scoop_identity::PersistentTypeId, Error = E>
        + PersistentIdResolver<scoop_identity::PersistentGenericTypeId, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PublicMemberRefV1 {
    Callable(CallableDeclarationId),
    Property(PropertyDeclarationId),
}

impl WireEncode for PublicMemberRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 1,
            Self::Property(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(declaration) => declaration.encode(encoder),
            Self::Property(declaration) => declaration.encode(encoder),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedPublicMemberRefV1 {
    Callable(DecodedCallableDeclarationId),
    Property(DecodedPropertyDeclarationId),
}

impl DecodedPublicMemberRefV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<PublicMemberRefV1, E>
    where
        R: PublicMemberRefResolver<E>,
    {
        match self {
            Self::Callable(declaration) => declaration
                .resolve(resolver)
                .map(PublicMemberRefV1::Callable),
            Self::Property(declaration) => declaration
                .resolve(resolver)
                .map(PublicMemberRefV1::Property),
        }
    }
}

impl WireEncode for DecodedPublicMemberRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Callable(_) => 1,
            Self::Property(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Callable(declaration) => declaration.encode(encoder),
            Self::Property(declaration) => declaration.encode(encoder),
        }
    }
}

impl WireDecode for DecodedPublicMemberRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        if fields != 2 {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        match tag {
            1 => decoder
                .field(1, DecodedCallableDeclarationId::decode)
                .map(Self::Callable),
            2 => decoder
                .field(1, DecodedPropertyDeclarationId::decode)
                .map(Self::Property),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPublicMemberRefsV1 {
    members: Vec<PublicMemberRefV1>,
}

impl CanonicalPublicMemberRefsV1 {
    pub fn try_new(mut members: Vec<PublicMemberRefV1>) -> Result<Self, PublicMemberRefBuildError> {
        members.sort_unstable();
        if let Some(pair) = members.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(PublicMemberRefBuildError::Duplicate(pair[0]));
        }
        Ok(Self { members })
    }

    pub fn members(&self) -> &[PublicMemberRefV1] {
        &self.members
    }
}

impl WireEncode for CanonicalPublicMemberRefsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.members.len() as u64)?;
        for member in &self.members {
            member.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalPublicMemberRefsV1 {
    members: Vec<DecodedPublicMemberRefV1>,
}

impl DecodedCanonicalPublicMemberRefsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalPublicMemberRefsV1, PublicMemberRefSetValidationError<E>>
    where
        R: PublicMemberRefResolver<E>,
    {
        let mut members = Vec::<PublicMemberRefV1>::with_capacity(self.members.len());
        for (index, member) in self.members.into_iter().enumerate() {
            let member = member
                .resolve(resolver)
                .map_err(|error| PublicMemberRefSetValidationError::Member { index, error })?;
            if let Some(previous) = members.last() {
                match previous.cmp(&member) {
                    std::cmp::Ordering::Equal => {
                        return Err(PublicMemberRefSetValidationError::Duplicate { index, member });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(PublicMemberRefSetValidationError::NonCanonicalOrder { index });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            members.push(member);
        }
        Ok(CanonicalPublicMemberRefsV1 { members })
    }
}

impl WireEncode for DecodedCanonicalPublicMemberRefsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.members.len() as u64)?;
        for member in &self.members {
            member.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalPublicMemberRefsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedPublicMemberRefV1::decode(decoder))
            .map(|members| Self { members })
    }
}

pub trait CallableDeclarationIdResolver<E>:
    PersistentIdResolver<PersistentFunctionId, Error = E>
    + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
    + PersistentIdResolver<PersistentConstructorId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
{
}

impl<R, E> CallableDeclarationIdResolver<E> for R where
    R: PersistentIdResolver<PersistentFunctionId, Error = E>
        + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
        + PersistentIdResolver<PersistentConstructorId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
{
}

pub trait PropertyDeclarationIdResolver<E>:
    PersistentIdResolver<PersistentPropertyId, Error = E>
    + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
{
}

impl<R, E> PropertyDeclarationIdResolver<E> for R where
    R: PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
{
}

pub trait PublicMemberRefResolver<E>:
    CallableDeclarationIdResolver<E> + PropertyDeclarationIdResolver<E>
{
}

impl<R, E> PublicMemberRefResolver<E> for R where
    R: CallableDeclarationIdResolver<E> + PropertyDeclarationIdResolver<E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicMemberRefBuildError {
    Duplicate(PublicMemberRefV1),
}

impl fmt::Display for PublicMemberRefBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(member) => write!(formatter, "duplicate public member {member:?}"),
        }
    }
}

impl std::error::Error for PublicMemberRefBuildError {}

#[derive(Debug)]
pub enum PublicMemberRefSetValidationError<E> {
    Member {
        index: usize,
        error: E,
    },
    Duplicate {
        index: usize,
        member: PublicMemberRefV1,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for PublicMemberRefSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Member { index, error } => {
                write!(formatter, "invalid public member {index}: {error}")
            }
            Self::Duplicate { index, member } => {
                write!(
                    formatter,
                    "duplicate public member {member:?} at index {index}"
                )
            }
            Self::NonCanonicalOrder { index } => {
                write!(
                    formatter,
                    "non-canonical public member order at index {index}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for PublicMemberRefSetValidationError<E> {}

#[cfg(test)]
mod tests;
