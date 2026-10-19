use super::*;

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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
