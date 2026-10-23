use super::*;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConditionalInterfaceApplicationV1 {
    pub declaration: ConditionalInterfaceId,
    pub owner: SignatureTypeKey,
}

impl WireEncode for ConditionalInterfaceApplicationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedConditionalInterfaceApplicationV1 {
    declaration: ConditionalInterfaceId,
    owner: DecodedSignatureTypeKey,
}

impl DecodedConditionalInterfaceApplicationV1 {
    pub fn resolve<R: NominalInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ConditionalInterfaceApplicationV1, E> {
        Ok(ConditionalInterfaceApplicationV1 {
            declaration: self.declaration,
            owner: self.owner.resolve(resolver)?,
        })
    }
}

impl WireDecode for DecodedConditionalInterfaceApplicationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            declaration: decoder.field(1, ConditionalInterfaceId::decode)?,
            owner: decoder.field(2, DecodedSignatureTypeKey::decode)?,
        })
    }
}

impl WireEncode for DecodedConditionalInterfaceApplicationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)
    }
}
