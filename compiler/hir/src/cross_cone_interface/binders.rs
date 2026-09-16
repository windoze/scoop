use std::collections::BTreeSet;

use scoop_identity::{
    CanonicalIdentifier, DecodedCanonicalIdentifier, DecodedOptionalSignatureType,
    DecodedSignatureTypeKey, OptionalSignatureType, PersistentGenericTypeId, PersistentIdResolver,
    PersistentTypeId, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

mod errors;
mod scope;

pub use errors::{
    BinderListValidationError, SignatureTypeSetBuildError, SignatureTypeSetValidationError,
    TypeParameterBinderBuildError, TypeParameterBinderResolutionError,
    TypeParameterBoundsBuildError, TypeParameterBoundsResolutionError,
};
pub use scope::{
    SignatureBinderScopeError, SignatureBinderScopeV1, TypeParameterBinderScopeValidationError,
    TypeParameterBoundLocation,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSignatureTypesV1 {
    values: Vec<SignatureTypeKey>,
}

impl CanonicalSignatureTypesV1 {
    pub fn try_new(mut values: Vec<SignatureTypeKey>) -> Result<Self, SignatureTypeSetBuildError> {
        values.sort_unstable();
        if let Some(pair) = values.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(SignatureTypeSetBuildError::Duplicate(Box::new(
                pair[0].clone(),
            )));
        }
        Ok(Self { values })
    }

    pub fn values(&self) -> &[SignatureTypeKey] {
        &self.values
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl WireEncode for CanonicalSignatureTypesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.values.len() as u64)?;
        for value in &self.values {
            value.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalSignatureTypesV1 {
    values: Vec<DecodedSignatureTypeKey>,
}

impl DecodedCanonicalSignatureTypesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalSignatureTypesV1, SignatureTypeSetValidationError<E>>
    where
        R: SignatureTypeReferenceResolver<E>,
    {
        let mut values = Vec::<SignatureTypeKey>::with_capacity(self.values.len());
        for (index, value) in self.values.into_iter().enumerate() {
            let value = value
                .resolve(resolver)
                .map_err(|error| SignatureTypeSetValidationError::Reference { index, error })?;
            if let Some(previous) = values.last() {
                match previous.cmp(&value) {
                    std::cmp::Ordering::Equal => {
                        return Err(SignatureTypeSetValidationError::Duplicate { index });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(SignatureTypeSetValidationError::NonCanonicalOrder { index });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            values.push(value);
        }
        Ok(CanonicalSignatureTypesV1 { values })
    }
}

impl WireEncode for DecodedCanonicalSignatureTypesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.values.len() as u64)?;
        for value in &self.values {
            value.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalSignatureTypesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedSignatureTypeKey::decode(decoder))
            .map(|values| Self { values })
    }
}

pub trait SignatureTypeReferenceResolver<E>:
    PersistentIdResolver<PersistentTypeId, Error = E>
    + PersistentIdResolver<PersistentGenericTypeId, Error = E>
{
}

impl<R, E> SignatureTypeReferenceResolver<E> for R where
    R: PersistentIdResolver<PersistentTypeId, Error = E>
        + PersistentIdResolver<PersistentGenericTypeId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalTypeParameterBoundsV1 {
    class: Option<SignatureTypeKey>,
    interfaces: CanonicalSignatureTypesV1,
}

impl NominalTypeParameterBoundsV1 {
    pub fn try_new(
        class: Option<SignatureTypeKey>,
        interfaces: CanonicalSignatureTypesV1,
    ) -> Result<Self, TypeParameterBoundsBuildError> {
        if class.is_none() && interfaces.is_empty() {
            return Err(TypeParameterBoundsBuildError::EmptyNominal);
        }
        Ok(Self { class, interfaces })
    }

    pub fn class(&self) -> Option<&SignatureTypeKey> {
        self.class.as_ref()
    }

    pub const fn interfaces(&self) -> &CanonicalSignatureTypesV1 {
        &self.interfaces
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeParameterBoundsV1 {
    Unconstrained,
    Value,
    Ref,
    Nominal(NominalTypeParameterBoundsV1),
}

impl WireEncode for TypeParameterBoundsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Unconstrained => encode_empty_sum(encoder, 1),
            Self::Value => encode_empty_sum(encoder, 2),
            Self::Ref => encode_empty_sum(encoder, 3),
            Self::Nominal(bounds) => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(4)?;
                encoder.field(1)?;
                OptionalSignatureType::from_option(bounds.class.clone()).encode(encoder)?;
                encoder.field(2)?;
                bounds.interfaces.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalTypeParameterBoundsV1 {
    class: DecodedOptionalSignatureType,
    interfaces: DecodedCanonicalSignatureTypesV1,
}

impl DecodedNominalTypeParameterBoundsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalTypeParameterBoundsV1, TypeParameterBoundsResolutionError<E>>
    where
        R: SignatureTypeReferenceResolver<E>,
    {
        let class = self
            .class
            .resolve(resolver)
            .map_err(TypeParameterBoundsResolutionError::Reference)?;
        let class = match class {
            OptionalSignatureType::Absent => None,
            OptionalSignatureType::Present(value) => Some(*value),
        };
        let interfaces = self
            .interfaces
            .resolve(resolver)
            .map_err(TypeParameterBoundsResolutionError::Interfaces)?;
        NominalTypeParameterBoundsV1::try_new(class, interfaces)
            .map_err(TypeParameterBoundsResolutionError::Shape)
    }
}

impl WireEncode for DecodedNominalTypeParameterBoundsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(4)?;
        encoder.field(1)?;
        self.class.encode(encoder)?;
        encoder.field(2)?;
        self.interfaces.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedTypeParameterBoundsV1 {
    Unconstrained,
    Value,
    Ref,
    Nominal(DecodedNominalTypeParameterBoundsV1),
}

impl DecodedTypeParameterBoundsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<TypeParameterBoundsV1, TypeParameterBoundsResolutionError<E>>
    where
        R: SignatureTypeReferenceResolver<E>,
    {
        match self {
            Self::Unconstrained => Ok(TypeParameterBoundsV1::Unconstrained),
            Self::Value => Ok(TypeParameterBoundsV1::Value),
            Self::Ref => Ok(TypeParameterBoundsV1::Ref),
            Self::Nominal(bounds) => bounds.resolve(resolver).map(TypeParameterBoundsV1::Nominal),
        }
    }
}

impl WireEncode for DecodedTypeParameterBoundsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Unconstrained => encode_empty_sum(encoder, 1),
            Self::Value => encode_empty_sum(encoder, 2),
            Self::Ref => encode_empty_sum(encoder, 3),
            Self::Nominal(bounds) => bounds.encode(encoder),
        }
    }
}

impl WireDecode for DecodedTypeParameterBoundsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Unconstrained)
            }
            2 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Value)
            }
            3 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Ref)
            }
            4 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Nominal(DecodedNominalTypeParameterBoundsV1 {
                    class: decoder.field(1, DecodedOptionalSignatureType::decode)?,
                    interfaces: decoder.field(2, DecodedCanonicalSignatureTypesV1::decode)?,
                }))
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeParameterBinderV1 {
    name: CanonicalIdentifier,
    bounds: TypeParameterBoundsV1,
}

impl TypeParameterBinderV1 {
    pub const fn new(name: CanonicalIdentifier, bounds: TypeParameterBoundsV1) -> Self {
        Self { name, bounds }
    }

    pub const fn name(&self) -> &CanonicalIdentifier {
        &self.name
    }

    pub const fn bounds(&self) -> &TypeParameterBoundsV1 {
        &self.bounds
    }
}

impl WireEncode for TypeParameterBinderV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.name.encode(encoder)?;
        encoder.field(2)?;
        self.bounds.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTypeParameterBinderV1 {
    name: DecodedCanonicalIdentifier,
    bounds: DecodedTypeParameterBoundsV1,
}

impl DecodedTypeParameterBinderV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<TypeParameterBinderV1, TypeParameterBinderResolutionError<E>>
    where
        R: SignatureTypeReferenceResolver<E>,
    {
        let name = self
            .name
            .validate()
            .map_err(TypeParameterBinderResolutionError::Name)?;
        let bounds = self
            .bounds
            .resolve(resolver)
            .map_err(TypeParameterBinderResolutionError::Bounds)?;
        Ok(TypeParameterBinderV1 { name, bounds })
    }
}

impl WireEncode for DecodedTypeParameterBinderV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.name.encode(encoder)?;
        encoder.field(2)?;
        self.bounds.encode(encoder)
    }
}

impl WireDecode for DecodedTypeParameterBinderV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            name: decoder.field(1, DecodedCanonicalIdentifier::decode)?,
            bounds: decoder.field(2, DecodedTypeParameterBoundsV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalBinderListV1 {
    binders: Vec<TypeParameterBinderV1>,
}

impl CanonicalBinderListV1 {
    pub fn try_new(
        binders: Vec<TypeParameterBinderV1>,
    ) -> Result<Self, TypeParameterBinderBuildError> {
        u32::try_from(binders.len()).map_err(|_| TypeParameterBinderBuildError::TooMany)?;
        let mut names = BTreeSet::new();
        for binder in &binders {
            if !names.insert(binder.name().clone()) {
                return Err(TypeParameterBinderBuildError::DuplicateName(
                    binder.name().clone(),
                ));
            }
        }
        Ok(Self { binders })
    }

    pub fn binders(&self) -> &[TypeParameterBinderV1] {
        &self.binders
    }

    pub fn len_u32(&self) -> u32 {
        u32::try_from(self.binders.len())
            .expect("a canonical binder list length is validated at construction")
    }

    pub fn is_empty(&self) -> bool {
        self.binders.is_empty()
    }
}

impl WireEncode for CanonicalBinderListV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.binders.len() as u64)?;
        for binder in &self.binders {
            binder.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalBinderListV1 {
    binders: Vec<DecodedTypeParameterBinderV1>,
}

impl DecodedCanonicalBinderListV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalBinderListV1, BinderListValidationError<E>>
    where
        R: SignatureTypeReferenceResolver<E>,
    {
        u32::try_from(self.binders.len()).map_err(|_| BinderListValidationError::TooMany)?;
        let mut binders = Vec::with_capacity(self.binders.len());
        let mut names = BTreeSet::new();
        for (index, binder) in self.binders.into_iter().enumerate() {
            let binder = binder
                .resolve(resolver)
                .map_err(|error| BinderListValidationError::Binder { index, error })?;
            if !names.insert(binder.name().clone()) {
                return Err(BinderListValidationError::DuplicateName {
                    index,
                    name: binder.name().clone(),
                });
            }
            binders.push(binder);
        }
        Ok(CanonicalBinderListV1 { binders })
    }
}

impl WireEncode for DecodedCanonicalBinderListV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.binders.len() as u64)?;
        for binder in &self.binders {
            binder.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalBinderListV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedTypeParameterBinderV1::decode(decoder))
            .map(|binders| Self { binders })
    }
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
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
