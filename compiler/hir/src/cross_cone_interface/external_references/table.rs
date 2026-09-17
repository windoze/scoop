use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedExternalHirReferenceV1, ExternalHirReferenceResolutionError,
    ExternalHirReferenceResolver, ExternalHirReferenceV1, ExternalHirTargetV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExternalHirReferencesV1 {
    records: Vec<ExternalHirReferenceV1>,
}

impl CanonicalExternalHirReferencesV1 {
    pub fn try_new(
        mut records: Vec<ExternalHirReferenceV1>,
    ) -> Result<Self, ExternalHirReferenceSetBuildError> {
        records.sort_unstable_by_key(ExternalHirReferenceV1::target);
        if let Some(target) = records
            .windows(2)
            .find(|pair| pair[0].target() == pair[1].target())
            .map(|pair| pair[0].target())
        {
            return Err(ExternalHirReferenceSetBuildError::DuplicateTarget(target));
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[ExternalHirReferenceV1] {
        &self.records
    }

    pub fn get(&self, target: ExternalHirTargetV1) -> Option<&ExternalHirReferenceV1> {
        self.records
            .binary_search_by_key(&target, ExternalHirReferenceV1::target)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl WireEncode for CanonicalExternalHirReferencesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalExternalHirReferencesV1 {
    records: Vec<DecodedExternalHirReferenceV1>,
}

impl DecodedCanonicalExternalHirReferencesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalExternalHirReferencesV1, ExternalHirReferenceSetValidationError<E>>
    where
        R: ExternalHirReferenceResolver<E>,
    {
        let mut records = Vec::<ExternalHirReferenceV1>::with_capacity(self.records.len());
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record
                .resolve(resolver)
                .map_err(|error| ExternalHirReferenceSetValidationError::Record { index, error })?;
            if let Some(previous) = records.last() {
                match previous.target().cmp(&record.target()) {
                    std::cmp::Ordering::Equal => {
                        return Err(ExternalHirReferenceSetValidationError::DuplicateTarget {
                            index,
                            target: record.target(),
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(ExternalHirReferenceSetValidationError::NonCanonicalOrder {
                            index,
                        });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            records.push(record);
        }
        Ok(CanonicalExternalHirReferencesV1 { records })
    }
}

impl WireEncode for DecodedCanonicalExternalHirReferencesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalExternalHirReferencesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExternalHirReferenceV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalHirReferenceSetBuildError {
    DuplicateTarget(ExternalHirTargetV1),
}

impl fmt::Display for ExternalHirReferenceSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateTarget(target) => {
                write!(formatter, "duplicate external HIR target {target:?}")
            }
        }
    }
}

impl std::error::Error for ExternalHirReferenceSetBuildError {}

#[derive(Debug)]
pub enum ExternalHirReferenceSetValidationError<E> {
    Record {
        index: usize,
        error: ExternalHirReferenceResolutionError<E>,
    },
    DuplicateTarget {
        index: usize,
        target: ExternalHirTargetV1,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for ExternalHirReferenceSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(formatter, "invalid external HIR reference {index}: {error}")
            }
            Self::DuplicateTarget { index, target } => {
                write!(
                    formatter,
                    "duplicate external HIR target {target:?} at index {index}"
                )
            }
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical external HIR reference order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExternalHirReferenceSetValidationError<E>
{
}

#[cfg(test)]
mod tests;
