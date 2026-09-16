use std::fmt;

use scoop_identity::{DecodedPersistentId, PersistentId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPersistentIdsV1<I: PersistentId> {
    values: Vec<I>,
}

impl<I: PersistentId> CanonicalPersistentIdsV1<I> {
    pub fn try_new(mut values: Vec<I>) -> Result<Self, CanonicalPersistentIdSetBuildError<I>> {
        values.sort_unstable();
        if let Some(pair) = values.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(CanonicalPersistentIdSetBuildError::Duplicate(pair[0]));
        }
        Ok(Self { values })
    }

    pub fn values(&self) -> &[I] {
        &self.values
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<I> WireEncode for CanonicalPersistentIdsV1<I>
where
    I: PersistentId + WireEncode,
{
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.values.len() as u64)?;
        for value in &self.values {
            value.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalPersistentIdsV1<I: PersistentId> {
    values: Vec<DecodedPersistentId<I>>,
}

impl<I: PersistentId> DecodedCanonicalPersistentIdsV1<I> {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalPersistentIdsV1<I>, CanonicalPersistentIdSetValidationError<I, E>>
    where
        R: PersistentIdResolver<I, Error = E>,
    {
        let mut values = Vec::<I>::with_capacity(self.values.len());
        for (index, value) in self.values.into_iter().enumerate() {
            let value = resolver.resolve(value).map_err(|error| {
                CanonicalPersistentIdSetValidationError::Reference { index, error }
            })?;
            if let Some(previous) = values.last() {
                match previous.cmp(&value) {
                    std::cmp::Ordering::Equal => {
                        return Err(CanonicalPersistentIdSetValidationError::Duplicate {
                            index,
                            id: value,
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(CanonicalPersistentIdSetValidationError::NonCanonicalOrder {
                            index,
                        });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            values.push(value);
        }
        Ok(CanonicalPersistentIdsV1 { values })
    }
}

impl<I: PersistentId> WireEncode for DecodedCanonicalPersistentIdsV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.values.len() as u64)?;
        for value in &self.values {
            value.encode(encoder)?;
        }
        Ok(())
    }
}

impl<I: PersistentId> WireDecode for DecodedCanonicalPersistentIdsV1<I> {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            .map(|values| Self { values })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalPersistentIdSetBuildError<I: PersistentId> {
    Duplicate(I),
}

impl<I: PersistentId + fmt::Display> fmt::Display for CanonicalPersistentIdSetBuildError<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(id) => write!(formatter, "duplicate {} identity {id}", I::KIND),
        }
    }
}

impl<I: PersistentId + fmt::Display> std::error::Error for CanonicalPersistentIdSetBuildError<I> {}

#[derive(Debug, Eq, PartialEq)]
pub enum CanonicalPersistentIdSetValidationError<I: PersistentId, E> {
    Reference { index: usize, error: E },
    Duplicate { index: usize, id: I },
    NonCanonicalOrder { index: usize },
}

impl<I: PersistentId + fmt::Display, E: fmt::Display> fmt::Display
    for CanonicalPersistentIdSetValidationError<I, E>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference { index, error } => {
                write!(
                    formatter,
                    "invalid {} identity at index {index}: {error}",
                    I::KIND
                )
            }
            Self::Duplicate { index, id } => {
                write!(
                    formatter,
                    "duplicate {} identity {id} at index {index}",
                    I::KIND
                )
            }
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical {} identity order at index {index}",
                I::KIND
            ),
        }
    }
}

impl<I, E> std::error::Error for CanonicalPersistentIdSetValidationError<I, E>
where
    I: PersistentId + fmt::Display,
    E: std::error::Error + 'static,
{
}

#[cfg(test)]
mod tests;
