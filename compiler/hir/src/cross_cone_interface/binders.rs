use std::collections::BTreeSet;

use scoop_identity::{
    CanonicalIdentifier, DecodedCanonicalIdentifier, DecodedOptionalSignatureType,
    DecodedSignatureTypeKey, OptionalSignatureType, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::SourceNominalIdResolver;

mod bounds;
pub use bounds::{
    DecodedNominalTypeParameterBoundsV1, DecodedTypeParameterBoundsV1,
    NominalTypeParameterBoundsV1, TypeParameterBoundsV1,
};
mod companions;
mod requirements;
pub use companions::{CompanionBoundsV1, DecodedCompanionBoundsV1};
mod errors;
mod scope;
mod semantics;

pub use errors::{
    BinderListValidationError, SignatureTypeSetBuildError, SignatureTypeSetValidationError,
    TypeParameterBinderBuildError, TypeParameterBinderResolutionError,
    TypeParameterBoundsBuildError, TypeParameterBoundsResolutionError,
};
pub use scope::{
    SignatureBinderScopeError, SignatureBinderScopeV1, TypeParameterBinderScopeValidationError,
    TypeParameterBoundLocation,
};
pub use semantics::{
    NominalBoundSemanticError, NominalInterfaceShapeAuthority, NominalSignatureSemanticError,
    PublicNominalShapeV1, SignatureTypeFormV1, SignatureTypeSemanticError,
    TypeParameterBinderSemanticValidationError,
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedSignatureTypeKey::decode(decoder))
            .map(|values| Self { values })
    }
}

pub trait SignatureTypeReferenceResolver<E>: SourceNominalIdResolver<E> {}

impl<R, E> SignatureTypeReferenceResolver<E> for R where R: SourceNominalIdResolver<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeParameterBinderV1 {
    name: CanonicalIdentifier,
    bounds: TypeParameterBoundsV1,
    excluded_interfaces: CanonicalSignatureTypesV1,
    companion: Option<CompanionBoundsV1>,
    requirements: Vec<scoop_identity::MemberRequirementKey>,
}

impl TypeParameterBinderV1 {
    pub const fn new(name: CanonicalIdentifier, bounds: TypeParameterBoundsV1) -> Self {
        Self {
            name,
            bounds,
            excluded_interfaces: CanonicalSignatureTypesV1 { values: Vec::new() },
            companion: None,
            requirements: Vec::new(),
        }
    }

    pub fn with_requirements(
        mut self,
        mut requirements: Vec<scoop_identity::MemberRequirementKey>,
    ) -> Self {
        requirements.sort_unstable();
        self.requirements = requirements;
        self
    }

    pub fn requirements(&self) -> &[scoop_identity::MemberRequirementKey] {
        &self.requirements
    }

    pub fn with_companion(mut self, companion: Option<CompanionBoundsV1>) -> Self {
        self.companion = companion;
        self
    }

    pub fn companion(&self) -> Option<&CompanionBoundsV1> {
        self.companion.as_ref()
    }

    pub fn with_excluded_interfaces(mut self, excluded: CanonicalSignatureTypesV1) -> Self {
        self.excluded_interfaces = excluded;
        self
    }

    pub fn excluded_interfaces(&self) -> &CanonicalSignatureTypesV1 {
        &self.excluded_interfaces
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
        encoder.map(
            2 + u64::from(
                !self.excluded_interfaces.values.is_empty()
                    || self.companion.is_some()
                    || !self.requirements.is_empty(),
            ) + u64::from(self.companion.is_some())
                + u64::from(!self.requirements.is_empty()),
        )?;
        encoder.field(1)?;
        self.name.encode(encoder)?;
        encoder.field(2)?;
        self.bounds.encode(encoder)?;
        if !self.excluded_interfaces.values.is_empty()
            || self.companion.is_some()
            || !self.requirements.is_empty()
        {
            encoder.field(3)?;
            self.excluded_interfaces.encode(encoder)?;
        }
        if let Some(companion) = &self.companion {
            encoder.field(4)?;
            companion.encode(encoder)?;
        }
        requirements::encode(&self.requirements, encoder)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTypeParameterBinderV1 {
    name: DecodedCanonicalIdentifier,
    bounds: DecodedTypeParameterBoundsV1,
    excluded_interfaces: DecodedCanonicalSignatureTypesV1,
    companion: Option<DecodedCompanionBoundsV1>,
    requirements: Vec<scoop_identity::MemberRequirementKey<DecodedSignatureTypeKey>>,
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
        let excluded_interfaces = self
            .excluded_interfaces
            .resolve(resolver)
            .map_err(TypeParameterBinderResolutionError::ExcludedInterfaces)?;
        Ok(TypeParameterBinderV1 {
            name,
            bounds,
            excluded_interfaces,
            requirements: requirements::resolve(self.requirements, resolver)?,
            companion: self
                .companion
                .map(|value| value.resolve(resolver))
                .transpose()?,
        })
    }
}

impl WireEncode for DecodedTypeParameterBinderV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(
            2 + u64::from(
                !self.excluded_interfaces.values.is_empty()
                    || self.companion.is_some()
                    || !self.requirements.is_empty(),
            ) + u64::from(self.companion.is_some())
                + u64::from(!self.requirements.is_empty()),
        )?;
        encoder.field(1)?;
        self.name.encode(encoder)?;
        encoder.field(2)?;
        self.bounds.encode(encoder)?;
        if !self.excluded_interfaces.values.is_empty()
            || self.companion.is_some()
            || !self.requirements.is_empty()
        {
            encoder.field(3)?;
            self.excluded_interfaces.encode(encoder)?;
        }
        if let Some(companion) = &self.companion {
            encoder.field(4)?;
            companion.encode(encoder)?;
        }
        requirements::encode(&self.requirements, encoder)?;
        Ok(())
    }
}

impl WireDecode for DecodedTypeParameterBinderV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if !(2..=5).contains(&fields) {
            return Err(wire_error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected: 2,
                    actual: fields,
                },
            ));
        }
        let name = decoder.field(1, DecodedCanonicalIdentifier::decode)?;
        let bounds = decoder.field(2, DecodedTypeParameterBoundsV1::decode)?;
        let excluded_interfaces = if fields >= 3 {
            decoder.field(3, DecodedCanonicalSignatureTypesV1::decode)?
        } else {
            DecodedCanonicalSignatureTypesV1 { values: Vec::new() }
        };
        let mut companion = None;
        let mut requirements = Vec::new();
        let mut previous = 3;
        for _ in 3..fields {
            let field = decoder.unsigned()?;
            if field <= previous {
                return Err(wire_error(decoder, WireErrorKind::NonCanonicalCbor));
            }
            match field {
                4 => companion = Some(DecodedCompanionBoundsV1::decode(decoder)?),
                5 => {
                    requirements = decoder.decode_array(|decoder, _| {
                        scoop_identity::MemberRequirementKey::decode(decoder)
                    })?
                }
                field => return Err(wire_error(decoder, WireErrorKind::ExtraField { field })),
            }
            previous = field;
        }
        Ok(Self {
            name,
            bounds,
            excluded_interfaces,
            companion,
            requirements,
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
