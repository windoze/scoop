use super::*;
use scoop_wire::{Decoder, WireDecode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalSourcePropertyPayloadV1(DecodedProtectedPropertyPayloadV1);
impl DecodedNominalSourcePropertyPayloadV1 {
    pub fn resolve<R: ProtectedPropertyInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalSourcePropertyPayloadV1, ProtectedPropertyResolutionError<E>> {
        self.0.resolve_source_payload(resolver)
    }
}
impl WireEncode for DecodedNominalSourcePropertyPayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}
impl WireDecode for DecodedNominalSourcePropertyPayloadV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        DecodedProtectedPropertyPayloadV1::decode(decoder).map(Self)
    }
}
