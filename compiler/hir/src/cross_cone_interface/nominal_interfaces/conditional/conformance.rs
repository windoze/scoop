use super::*;

/// A checked relation, including inherited relations and interfaces with no slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionalInterfaceConformanceV1 {
    pub interface: SignatureTypeKey,
    pub condition: ConditionalInterfaceApplicationV1,
}

impl WireEncode for ConditionalInterfaceConformanceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.interface.encode(encoder)?;
        encoder.field(2)?;
        self.condition.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedConditionalInterfaceConformanceV1 {
    interface: DecodedSignatureTypeKey,
    condition: DecodedConditionalInterfaceApplicationV1,
}

impl DecodedConditionalInterfaceConformanceV1 {
    pub fn resolve<R: NominalInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ConditionalInterfaceConformanceV1, E> {
        Ok(ConditionalInterfaceConformanceV1 {
            interface: self.interface.resolve(resolver)?,
            condition: self.condition.resolve(resolver)?,
        })
    }
}

impl WireDecode for DecodedConditionalInterfaceConformanceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            interface: decoder.field(1, DecodedSignatureTypeKey::decode)?,
            condition: decoder.field(2, DecodedConditionalInterfaceApplicationV1::decode)?,
        })
    }
}

impl WireEncode for DecodedConditionalInterfaceConformanceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.interface.encode(encoder)?;
        encoder.field(2)?;
        self.condition.encode(encoder)
    }
}
