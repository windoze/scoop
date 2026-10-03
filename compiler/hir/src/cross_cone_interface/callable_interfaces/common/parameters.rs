use std::collections::BTreeSet;

use scoop_identity::{
    CanonicalIdentifier, DecodedCanonicalIdentifier, DecodedSignatureTypeKey, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    SourceParameterListBuildError, SourceParameterListValidationError,
    SourceParameterShapeResolutionError,
};
use crate::SignatureTypeReferenceResolver;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceParameterShapeV1 {
    name: CanonicalIdentifier,
    value_type: SignatureTypeKey,
}

impl SourceParameterShapeV1 {
    pub const fn new(name: CanonicalIdentifier, value_type: SignatureTypeKey) -> Self {
        Self { name, value_type }
    }

    pub const fn name(&self) -> &CanonicalIdentifier {
        &self.name
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
}

impl WireEncode for SourceParameterShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.name.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSourceParameterShapeV1 {
    name: DecodedCanonicalIdentifier,
    value_type: DecodedSignatureTypeKey,
}

impl DecodedSourceParameterShapeV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<SourceParameterShapeV1, SourceParameterShapeResolutionError<E>>
    where
        R: SignatureTypeReferenceResolver<E>,
    {
        let name = self
            .name
            .validate()
            .map_err(SourceParameterShapeResolutionError::Name)?;
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(SourceParameterShapeResolutionError::ValueType)?;
        Ok(SourceParameterShapeV1 { name, value_type })
    }
}

impl WireEncode for DecodedSourceParameterShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.name.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)
    }
}

impl WireDecode for DecodedSourceParameterShapeV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            name: decoder.field(1, DecodedCanonicalIdentifier::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSourceParameterShapesV1 {
    parameters: Vec<SourceParameterShapeV1>,
    len: u32,
}

impl CanonicalSourceParameterShapesV1 {
    pub fn try_new(
        parameters: Vec<SourceParameterShapeV1>,
    ) -> Result<Self, SourceParameterListBuildError> {
        let len =
            u32::try_from(parameters.len()).map_err(|_| SourceParameterListBuildError::TooMany)?;
        let mut names = BTreeSet::new();
        for parameter in &parameters {
            if !names.insert(parameter.name.clone()) {
                return Err(SourceParameterListBuildError::DuplicateName(
                    parameter.name.clone(),
                ));
            }
        }
        Ok(Self { parameters, len })
    }

    pub fn parameters(&self) -> &[SourceParameterShapeV1] {
        &self.parameters
    }

    pub const fn len_u32(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }
}

impl WireEncode for CanonicalSourceParameterShapesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(u64::from(self.len))?;
        for parameter in &self.parameters {
            parameter.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalSourceParameterShapesV1 {
    parameters: Vec<DecodedSourceParameterShapeV1>,
}

impl DecodedCanonicalSourceParameterShapesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalSourceParameterShapesV1, SourceParameterListValidationError<E>>
    where
        R: SignatureTypeReferenceResolver<E>,
    {
        let len = u32::try_from(self.parameters.len())
            .map_err(|_| SourceParameterListValidationError::TooMany)?;
        let mut parameters = Vec::with_capacity(self.parameters.len());
        let mut names = BTreeSet::new();
        for (index, parameter) in self.parameters.into_iter().enumerate() {
            let parameter = parameter
                .resolve(resolver)
                .map_err(|error| SourceParameterListValidationError::Parameter { index, error })?;
            if !names.insert(parameter.name.clone()) {
                return Err(SourceParameterListValidationError::DuplicateName {
                    index,
                    name: parameter.name,
                });
            }
            parameters.push(parameter);
        }
        Ok(CanonicalSourceParameterShapesV1 { parameters, len })
    }
}

impl WireEncode for DecodedCanonicalSourceParameterShapesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.parameters.len() as u64)?;
        for parameter in &self.parameters {
            parameter.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalSourceParameterShapesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedSourceParameterShapeV1::decode(decoder))
            .map(|parameters| Self { parameters })
    }
}
