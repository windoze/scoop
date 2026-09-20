use super::super::parameter_protocols::wire::DecodedProtocol;
use super::*;
use crate::ProtectedCallableInterfaceResolver;
use scoop_wire::{Decoder, WireDecode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalSourceParameterProtocolsV1 {
    records: Vec<DecodedProtocol>,
}
impl DecodedCanonicalNominalSourceParameterProtocolsV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalNominalSourceParameterProtocolsV1, SourceInventoryError> {
        let mut records = reserve(self.records.len(), meter)?;
        for record in self.records {
            records.push(record.resolve(resolver, meter)?);
        }
        CanonicalNominalSourceParameterProtocolsV1::from_ordered(records, meter)
    }
}
impl WireDecode for DecodedCanonicalNominalSourceParameterProtocolsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedProtocol::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalNominalSourceParameterProtocolsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::super::wire::sequence(encoder, &self.records)
    }
}
