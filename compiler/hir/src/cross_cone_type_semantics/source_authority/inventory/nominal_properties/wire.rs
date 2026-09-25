use super::*;
use crate::{
    DecodedNominalSupportPropertyInterfaceV1, NominalSupportPropertyResolutionError,
    ProtectedPropertyInterfaceResolver,
};
use scoop_wire::{Decoder, WireDecode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalSourcePropertiesV1 {
    records: Vec<DecodedNominalSupportPropertyInterfaceV1>,
}
impl WireDecode for DecodedCanonicalNominalSourcePropertiesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedNominalSupportPropertyInterfaceV1::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalNominalSourcePropertiesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::super::wire::sequence(encoder, &self.records)
    }
}
impl DecodedCanonicalNominalSourcePropertiesV1 {
    pub fn resolve<R: ProtectedPropertyInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalNominalSourcePropertiesV1, NominalSourcePropertyResolutionError<E>> {
        use NominalSourcePropertyResolutionError as Error;
        let mut records = reserve(self.records.len()).map_err(Error::Inventory)?;
        for record in self.records {
            records.push(record.resolve(resolver).map_err(Error::Contract)?);
        }
        CanonicalNominalSourcePropertiesV1::from_ordered(records).map_err(Error::Inventory)
    }
}
#[derive(Debug)]
pub enum NominalSourcePropertyResolutionError<E> {
    Inventory(SourceInventoryError),
    Contract(NominalSupportPropertyResolutionError<E>),
}
impl<E: fmt::Display> fmt::Display for NominalSourcePropertyResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inventory(error) => error.fmt(f),
            Self::Contract(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for NominalSourcePropertyResolutionError<E> {}
