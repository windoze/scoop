use super::*;

/// A required public companion and its independently constrained type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompanionBoundsV1 {
    bounds: TypeParameterBoundsV1,
    excluded_interfaces: CanonicalSignatureTypesV1,
}

impl CompanionBoundsV1 {
    pub fn new(
        bounds: TypeParameterBoundsV1,
        excluded_interfaces: CanonicalSignatureTypesV1,
    ) -> Self {
        Self {
            bounds,
            excluded_interfaces,
        }
    }

    pub fn bounds(&self) -> &TypeParameterBoundsV1 {
        &self.bounds
    }

    pub fn excluded_interfaces(&self) -> &CanonicalSignatureTypesV1 {
        &self.excluded_interfaces
    }
}

impl WireEncode for CompanionBoundsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.bounds.encode(encoder)?;
        encoder.field(2)?;
        self.excluded_interfaces.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCompanionBoundsV1 {
    bounds: DecodedTypeParameterBoundsV1,
    excluded_interfaces: DecodedCanonicalSignatureTypesV1,
}

impl DecodedCompanionBoundsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CompanionBoundsV1, TypeParameterBinderResolutionError<E>>
    where
        R: SignatureTypeReferenceResolver<E>,
    {
        Ok(CompanionBoundsV1 {
            bounds: self
                .bounds
                .resolve(resolver)
                .map_err(TypeParameterBinderResolutionError::Bounds)?,
            excluded_interfaces: self
                .excluded_interfaces
                .resolve(resolver)
                .map_err(TypeParameterBinderResolutionError::ExcludedInterfaces)?,
        })
    }
}

impl WireDecode for DecodedCompanionBoundsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        expect_sum_length(decoder, fields, 2)?;
        Ok(Self {
            bounds: decoder.field(1, DecodedTypeParameterBoundsV1::decode)?,
            excluded_interfaces: decoder.field(2, DecodedCanonicalSignatureTypesV1::decode)?,
        })
    }
}

impl WireEncode for DecodedCompanionBoundsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.bounds.encode(encoder)?;
        encoder.field(2)?;
        self.excluded_interfaces.encode(encoder)
    }
}
