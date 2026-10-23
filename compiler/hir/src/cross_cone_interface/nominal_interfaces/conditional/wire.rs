use super::*;

impl WireEncode for ConditionalInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        self.interface.encode(encoder)?;
        encoder.field(3)?;
        self.parameters.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedConditionalInterfaceRecordV1 {
    id: ConditionalInterfaceId,
    interface: DecodedSignatureTypeKey,
    parameters: DecodedCanonicalBinderListV1,
}

impl DecodedConditionalInterfaceRecordV1 {
    pub fn resolve<R: NominalInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ConditionalInterfaceRecordV1, ConditionalInterfaceRecordResolutionError<E>> {
        Ok(ConditionalInterfaceRecordV1::new(
            self.id,
            self.interface
                .resolve(resolver)
                .map_err(ConditionalInterfaceRecordResolutionError::Interface)?,
            self.parameters
                .resolve(resolver)
                .map_err(ConditionalInterfaceRecordResolutionError::Parameters)?,
        ))
    }
}

impl WireDecode for DecodedConditionalInterfaceRecordV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            id: decoder.field(1, ConditionalInterfaceId::decode)?,
            interface: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            parameters: decoder.field(3, DecodedCanonicalBinderListV1::decode)?,
        })
    }
}

impl WireEncode for DecodedConditionalInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        self.interface.encode(encoder)?;
        encoder.field(3)?;
        self.parameters.encode(encoder)
    }
}

#[derive(Debug)]
pub enum ConditionalInterfaceRecordResolutionError<E> {
    Interface(E),
    Parameters(crate::BinderListValidationError<E>),
}

impl<E: std::fmt::Display> std::fmt::Display for ConditionalInterfaceRecordResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interface(error) => error.fmt(f),
            Self::Parameters(error) => error.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ConditionalInterfaceRecordResolutionError<E>
{
}
