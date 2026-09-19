use std::fmt;

use scoop_identity::PersistentTypeId;
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::{
    DecodedNominalRepresentationSupportV1, NominalRepresentationResolutionError,
    NominalRepresentationResolver, NominalRepresentationSupportV1,
};
use crate::cross_cone_type_semantics::wire;

mod metered;

/// Canonical storage, not a substitute for source/facts/inheritance closure
/// validation. The section validator supplies that authority before selection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNominalRepresentationSupportV1 {
    records: Vec<NominalRepresentationSupportV1>,
}

impl CanonicalNominalRepresentationSupportV1 {
    pub fn try_new(
        mut records: Vec<NominalRepresentationSupportV1>,
    ) -> Result<Self, NominalRepresentationTableOrderError> {
        records.sort_unstable_by_key(NominalRepresentationSupportV1::owner);
        validate_order(&records)?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[NominalRepresentationSupportV1] {
        &self.records
    }
    pub fn get(&self, owner: PersistentTypeId) -> Option<&NominalRepresentationSupportV1> {
        self.records
            .binary_search_by_key(&owner, NominalRepresentationSupportV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
}

impl WireEncode for CanonicalNominalRepresentationSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNominalRepresentationSupportV1 {
    records: Vec<DecodedNominalRepresentationSupportV1>,
}

impl DecodedCanonicalNominalRepresentationSupportV1 {
    pub fn resolve<R: NominalRepresentationResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalNominalRepresentationSupportV1, NominalRepresentationTableResolutionError<E>>
    {
        self.resolve_metered(resolver, meter, &WirePath::root())
    }
}

impl WireEncode for DecodedCanonicalNominalRepresentationSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}
impl WireDecode for DecodedCanonicalNominalRepresentationSupportV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedNominalRepresentationSupportV1::decode(decoder))
            .map(|records| Self { records })
    }
}

fn validate_order(
    records: &[NominalRepresentationSupportV1],
) -> Result<(), NominalRepresentationTableOrderError> {
    for (index, pair) in records.windows(2).enumerate() {
        if pair[0].owner() >= pair[1].owner() {
            return Err(NominalRepresentationTableOrderError {
                index: index + 1,
                owner: pair[1].owner(),
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NominalRepresentationTableOrderError {
    pub index: usize,
    pub owner: PersistentTypeId,
}
impl fmt::Display for NominalRepresentationTableOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "duplicate or noncanonical representation owner {} at index {}",
            self.owner, self.index
        )
    }
}
impl std::error::Error for NominalRepresentationTableOrderError {}

#[derive(Debug)]
pub enum NominalRepresentationTableResolutionError<E> {
    Resource(WireError),
    Record {
        index: usize,
        source: NominalRepresentationResolutionError<E>,
    },
    Order(NominalRepresentationTableOrderError),
}
impl<E: fmt::Display> fmt::Display for NominalRepresentationTableResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Record { index, source } => write!(
                f,
                "invalid representation record at index {index}: {source}"
            ),
            Self::Order(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for NominalRepresentationTableResolutionError<E>
{
}

#[cfg(test)]
mod tests;
