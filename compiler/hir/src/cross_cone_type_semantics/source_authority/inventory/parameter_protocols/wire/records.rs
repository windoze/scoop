use super::*;

impl WireEncode for InheritanceSourceParameterV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.shape.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(tag(self.calling).into())?;
        encoder.field(3)?;
        self.origin.encode(encoder)
    }
}
impl WireEncode for InheritanceSourceParameterProtocolV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.source.encode(encoder)
    }
}
impl WireDecode for DecodedParameter {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            shape: decoder.field(1, DecodedSourceParameterShapeV1::decode)?,
            calling: decoder.field(2, calling)?,
            origin: decoder.field(3, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}
impl WireEncode for DecodedParameter {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.shape.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(tag(self.calling).into())?;
        encoder.field(3)?;
        self.origin.encode(encoder)
    }
}
impl WireDecode for DecodedProtocol {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            owner: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            parameters: decoder.field(2, |d| d.decode_array(|d, _| DecodedParameter::decode(d)))?,
        })
    }
}
impl WireEncode for DecodedProtocol {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        transport::sequence(encoder, &self.parameters)
    }
}
