use std::fmt;

use scoop_identity::PersistentPropertyId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedExportConstValueV1, ExportConstValueResolutionError, ExportConstValueResolver,
    ExportConstValueSemanticAuthority, ExportConstValueSemanticValidationError, ExportConstValueV1,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalExportConstValuesV1 {
    records: Vec<ExportConstValueV1>,
}

impl CanonicalExportConstValuesV1 {
    pub fn try_new(
        mut records: Vec<ExportConstValueV1>,
    ) -> Result<Self, ExportConstValueSetBuildError> {
        records.sort_unstable_by_key(ExportConstValueV1::property);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].property() == pair[1].property())
        {
            return Err(ExportConstValueSetBuildError::DuplicateProperty(
                pair[0].property(),
            ));
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[ExportConstValueV1] {
        &self.records
    }

    pub fn get(&self, property: PersistentPropertyId) -> Option<&ExportConstValueV1> {
        self.records
            .binary_search_by_key(&property, ExportConstValueV1::property)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), ExportConstValueSetSemanticValidationError<E>>
    where
        A: ExportConstValueSemanticAuthority<E>,
    {
        for (index, record) in self.records.iter().enumerate() {
            record.validate_semantics(authority).map_err(|error| {
                ExportConstValueSetSemanticValidationError::Record { index, error }
            })?;
        }
        Ok(())
    }
}

impl WireEncode for CanonicalExportConstValuesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalExportConstValuesV1 {
    records: Vec<DecodedExportConstValueV1>,
}

impl DecodedCanonicalExportConstValuesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalExportConstValuesV1, ExportConstValueSetValidationError<E>>
    where
        R: ExportConstValueResolver<E>,
    {
        let mut records = Vec::<ExportConstValueV1>::with_capacity(self.records.len());
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record
                .resolve(resolver)
                .map_err(|error| ExportConstValueSetValidationError::Record { index, error })?;
            if let Some(previous) = records.last() {
                match previous.property().cmp(&record.property()) {
                    std::cmp::Ordering::Equal => {
                        return Err(ExportConstValueSetValidationError::DuplicateProperty {
                            index,
                            property: record.property(),
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(ExportConstValueSetValidationError::NonCanonicalOrder {
                            index,
                        });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            records.push(record);
        }
        Ok(CanonicalExportConstValuesV1 { records })
    }
}

impl WireEncode for DecodedCanonicalExportConstValuesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalExportConstValuesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExportConstValueV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportConstValueSetBuildError {
    DuplicateProperty(PersistentPropertyId),
}

impl fmt::Display for ExportConstValueSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateProperty(property) => {
                write!(formatter, "duplicate exported const property {property}")
            }
        }
    }
}

impl std::error::Error for ExportConstValueSetBuildError {}

#[derive(Debug)]
pub enum ExportConstValueSetValidationError<E> {
    Record {
        index: usize,
        error: ExportConstValueResolutionError<E>,
    },
    DuplicateProperty {
        index: usize,
        property: PersistentPropertyId,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for ExportConstValueSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(formatter, "invalid exported const value {index}: {error}")
            }
            Self::DuplicateProperty { index, property } => write!(
                formatter,
                "duplicate exported const property {property} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical exported const value order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExportConstValueSetValidationError<E> {}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportConstValueSetSemanticValidationError<E> {
    Record {
        index: usize,
        error: ExportConstValueSemanticValidationError<E>,
    },
}

impl<E: fmt::Display> fmt::Display for ExportConstValueSetSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(
                    formatter,
                    "invalid exported const value semantics {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportConstValueSetSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
