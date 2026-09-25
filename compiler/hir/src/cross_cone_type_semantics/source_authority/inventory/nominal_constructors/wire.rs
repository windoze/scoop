use super::*;
use crate::{
    DecodedNominalSupportConstructorInterfaceV1, ProtectedCallableInterfaceResolutionError,
    ProtectedCallableInterfaceResolver,
};
use scoop_wire::{Decoder, WireDecode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalSourceConstructorsV1 {
    records: Vec<DecodedNominalSupportConstructorInterfaceV1>,
}

impl WireDecode for DecodedCanonicalNominalSourceConstructorsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedNominalSupportConstructorInterfaceV1::decode(d))
            .map(|records| Self { records })
    }
}

impl WireEncode for DecodedCanonicalNominalSourceConstructorsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::super::wire::sequence(encoder, &self.records)
    }
}

impl DecodedCanonicalNominalSourceConstructorsV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalNominalSourceConstructorsV1, NominalSourceConstructorResolutionError<E>>
    {
        use NominalSourceConstructorResolutionError as Error;
        let mut records = reserve(self.records.len()).map_err(Error::Inventory)?;
        for record in self.records {
            records.push(record.resolve(resolver).map_err(Error::Contract)?);
        }
        CanonicalNominalSourceConstructorsV1::from_ordered(records).map_err(Error::Inventory)
    }
}

#[derive(Debug)]
pub enum NominalSourceConstructorResolutionError<E> {
    Inventory(SourceInventoryError),
    Contract(ProtectedCallableInterfaceResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for NominalSourceConstructorResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inventory(error) => error.fmt(f),
            Self::Contract(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for NominalSourceConstructorResolutionError<E>
{
}
