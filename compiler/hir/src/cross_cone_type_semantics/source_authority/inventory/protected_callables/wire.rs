use super::*;
use crate::{
    DecodedProtectedCallableInterfaceV1, ProtectedCallableInterfaceResolutionError,
    ProtectedCallableInterfaceResolver,
};
use scoop_wire::{Decoder, WireDecode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalInheritanceSourceProtectedCallablesV1 {
    records: Vec<DecodedProtectedCallableInterfaceV1>,
}

impl WireDecode for DecodedCanonicalInheritanceSourceProtectedCallablesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedProtectedCallableInterfaceV1::decode(d))
            .map(|records| Self { records })
    }
}

impl WireEncode for DecodedCanonicalInheritanceSourceProtectedCallablesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::super::wire::sequence(encoder, &self.records)
    }
}

impl DecodedCanonicalInheritanceSourceProtectedCallablesV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<
        CanonicalInheritanceSourceProtectedCallablesV1,
        InheritanceSourceProtectedCallableResolutionError<E>,
    > {
        use InheritanceSourceProtectedCallableResolutionError as Error;
        let mut records = reserve(self.records.len(), meter).map_err(Error::Inventory)?;
        for record in self.records {
            records.push(record.resolve(resolver, meter).map_err(Error::Contract)?);
        }
        CanonicalInheritanceSourceProtectedCallablesV1::from_ordered(records, meter)
            .map_err(Error::Inventory)
    }
}

#[derive(Debug)]
pub enum InheritanceSourceProtectedCallableResolutionError<E> {
    Inventory(SourceInventoryError),
    Contract(ProtectedCallableInterfaceResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for InheritanceSourceProtectedCallableResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inventory(error) => error.fmt(f),
            Self::Contract(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for InheritanceSourceProtectedCallableResolutionError<E>
{
}
