use super::*;
use scoop_identity::{
    DecodedPersistentId, DecodedPersistentSymbolRequest, PersistentIdResolver,
    ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

impl WireEncode for ExternalTypeDescriptor {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(4)?;
        self.required_definition.encode(encoder)
    }
}

#[derive(Clone, Debug)]
pub struct DecodedExternalTypeDescriptor {
    provider: DecodedPersistentId<ConeIdentity>,
    target: DecodedPersistentId<PersistentExactTypeId>,
    expected_symbol: DecodedPersistentSymbolRequest,
    required_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
}

impl DecodedExternalTypeDescriptor {
    pub fn validate(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ExternalTypeDescriptor, ExternalTypeDescriptorDecodeError> {
        let actual = encode(&self).map_err(ExternalTypeDescriptorDecodeError::Encode)?;
        let provider = identities
            .resolve(self.provider)
            .map_err(ExternalTypeDescriptorDecodeError::Provider)?;
        let target = identities
            .resolve(self.target)
            .map_err(ExternalTypeDescriptorDecodeError::Target)?;
        let expected = ExternalTypeDescriptor::new(provider, target)
            .map_err(ExternalTypeDescriptorDecodeError::Build)?;
        // Derived references need not be registered in the consumer's local
        // graph. Their complete canonical bytes must match the actual provider.
        if encode(&expected).map_err(ExternalTypeDescriptorDecodeError::Encode)? != actual {
            return Err(ExternalTypeDescriptorDecodeError::RecordMismatch);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedExternalTypeDescriptor {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(4)?;
        self.required_definition.encode(encoder)
    }
}
impl WireDecode for DecodedExternalTypeDescriptor {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            target: decoder.field(2, DecodedPersistentId::decode)?,
            expected_symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
            required_definition: decoder.field(4, DecodedPersistentId::decode)?,
        })
    }
}
