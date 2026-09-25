use std::fmt;

use scoop_identity::{PersistentExactTypeId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{DecodedExactTypeFactsV1, ExactTypeFactsResolutionError, ExactTypeFactsV1};
use crate::cross_cone_type_semantics::wire;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalExactTypeFactsV1 {
    records: Vec<ExactTypeFactsV1>,
}

impl CanonicalExactTypeFactsV1 {
    pub fn try_new(mut records: Vec<ExactTypeFactsV1>) -> Result<Self, ExactTypeFactsTableError> {
        records.sort_unstable_by_key(|record| record.exact());
        Self::from_ordered(records)
    }

    fn from_ordered(records: Vec<ExactTypeFactsV1>) -> Result<Self, ExactTypeFactsTableError> {
        for (index, pair) in records.windows(2).enumerate() {
            if pair[0].exact() >= pair[1].exact() {
                return Err(ExactTypeFactsTableError {
                    index: index + 1,
                    exact: pair[1].exact(),
                });
            }
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[ExactTypeFactsV1] {
        &self.records
    }

    pub fn get(&self, exact: PersistentExactTypeId) -> Option<&ExactTypeFactsV1> {
        self.records
            .binary_search_by_key(&exact, |record| record.exact())
            .ok()
            .map(|index| &self.records[index])
    }
}

impl WireEncode for CanonicalExactTypeFactsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalExactTypeFactsV1 {
    records: Vec<DecodedExactTypeFactsV1>,
}

impl DecodedCanonicalExactTypeFactsV1 {
    pub fn resolve<R: PersistentIdResolver<PersistentExactTypeId>>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalExactTypeFactsV1, ExactTypeFactsTableResolutionError<R::Error>> {
        use ExactTypeFactsTableResolutionError as Error;
        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(
            &mut records,
            self.records.len(),
            &scoop_wire::WirePath::root(),
        )
        .map_err(Error::Allocation)?;
        for (index, decoded) in self.records.into_iter().enumerate() {
            let record = decoded
                .resolve(resolver)
                .map_err(|error| Error::Record { index, error })?;
            if records
                .last()
                .is_some_and(|previous: &ExactTypeFactsV1| previous.exact() >= record.exact())
            {
                return Err(Error::Order(ExactTypeFactsTableError {
                    index,
                    exact: record.exact(),
                }));
            }
            records.push(record);
        }
        Ok(CanonicalExactTypeFactsV1 { records })
    }
}

impl WireEncode for DecodedCanonicalExactTypeFactsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.records)
    }
}

impl WireDecode for DecodedCanonicalExactTypeFactsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExactTypeFactsV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactTypeFactsTableError {
    pub index: usize,
    pub exact: PersistentExactTypeId,
}

impl fmt::Display for ExactTypeFactsTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "duplicate or noncanonical exact type facts at index {}: {}",
            self.index, self.exact
        )
    }
}
impl std::error::Error for ExactTypeFactsTableError {}

#[derive(Debug, Eq, PartialEq)]
pub enum ExactTypeFactsTableResolutionError<E> {
    Allocation(WireError),
    Record {
        index: usize,
        error: ExactTypeFactsResolutionError<E>,
    },
    Order(ExactTypeFactsTableError),
}

impl<E: fmt::Display> fmt::Display for ExactTypeFactsTableResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation(error) => error.fmt(f),
            Self::Record { index, error } => {
                write!(f, "invalid exact type facts at index {index}: {error}")
            }
            Self::Order(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ExactTypeFactsTableResolutionError<E> {}
