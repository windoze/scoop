use super::*;
use crate::{
    DecodedNominalSupportPropertyInterfaceV1, NominalSupportPropertyResolutionError,
    ProtectedPropertyInterfaceResolver,
};
use scoop_wire::{Decoder, WireDecode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalInheritanceSourcePropertiesV1 {
    records: Vec<DecodedNominalSupportPropertyInterfaceV1>,
}

impl WireDecode for DecodedCanonicalInheritanceSourcePropertiesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedNominalSupportPropertyInterfaceV1::decode(d))
            .map(|records| Self { records })
    }
}

impl WireEncode for DecodedCanonicalInheritanceSourcePropertiesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::super::wire::sequence(encoder, &self.records)
    }
}

impl DecodedCanonicalInheritanceSourcePropertiesV1 {
    pub fn resolve<R: ProtectedPropertyInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalInheritanceSourcePropertiesV1, InheritanceSourcePropertyResolutionError<E>>
    {
        use InheritanceSourcePropertyResolutionError as Error;
        let mut records = reserve(self.records.len(), meter).map_err(Error::Inventory)?;
        for record in self.records {
            records.push(record.resolve(resolver, meter).map_err(Error::Contract)?);
        }
        CanonicalInheritanceSourcePropertiesV1::from_ordered(records, meter)
            .map_err(Error::Inventory)
    }
}

#[derive(Debug)]
pub enum InheritanceSourcePropertyResolutionError<E> {
    Inventory(SourceInventoryError),
    Contract(NominalSupportPropertyResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for InheritanceSourcePropertyResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inventory(error) => error.fmt(f),
            Self::Contract(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for InheritanceSourcePropertyResolutionError<E>
{
}
