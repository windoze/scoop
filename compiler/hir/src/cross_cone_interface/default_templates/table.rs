use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    DecodedExportDefaultTemplateV1, ExportDefaultTemplateIndexError,
    ExportDefaultTemplateResolutionError, ExportDefaultTemplateV1, IndexedExportDefaultTemplateV1,
};
use crate::{
    DefaultStatementReferenceResolver, ExportDefaultTemplateIndexResolver,
    ExportDefaultTemplateKeyResolver, ExportDefaultTemplateKeyV1,
};

mod semantics;

pub use semantics::ExportDefaultTemplateSourceClosureValidationError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExportDefaultTemplatesV1 {
    records: Vec<ExportDefaultTemplateV1>,
    len: u32,
}

impl CanonicalExportDefaultTemplatesV1 {
    pub fn try_new(
        mut records: Vec<ExportDefaultTemplateV1>,
    ) -> Result<Self, ExportDefaultTemplateSetBuildError> {
        let len = u32::try_from(records.len())
            .map_err(|_| ExportDefaultTemplateSetBuildError::TooMany)?;
        records.sort_unstable_by_key(ExportDefaultTemplateV1::key);
        if let Some(pair) = records
            .windows(2)
            .find(|pair| pair[0].key() == pair[1].key())
        {
            return Err(ExportDefaultTemplateSetBuildError::DuplicateKey(
                pair[0].key(),
            ));
        }
        Ok(Self { records, len })
    }

    pub fn records(&self) -> &[ExportDefaultTemplateV1] {
        &self.records
    }

    pub const fn len_u32(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn get(&self, key: ExportDefaultTemplateKeyV1) -> Option<&ExportDefaultTemplateV1> {
        self.records
            .binary_search_by_key(&key, ExportDefaultTemplateV1::key)
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn index_of(&self, key: ExportDefaultTemplateKeyV1) -> Option<u32> {
        self.records
            .binary_search_by_key(&key, ExportDefaultTemplateV1::key)
            .ok()
            .and_then(|index| u32::try_from(index).ok())
    }

    pub fn index_locals(
        &self,
    ) -> Result<IndexedCanonicalExportDefaultTemplatesV1<'_>, ExportDefaultTemplateSetIndexError>
    {
        let mut records = Vec::with_capacity(self.records.len());
        for (index, record) in self.records.iter().enumerate() {
            records.push(
                record
                    .index_locals()
                    .map_err(|error| ExportDefaultTemplateSetIndexError::Record { index, error })?,
            );
        }
        Ok(IndexedCanonicalExportDefaultTemplatesV1 { records })
    }
}

impl ExportDefaultTemplateKeyResolver for CanonicalExportDefaultTemplatesV1 {
    type Error = ExportDefaultTemplateLookupError;

    fn resolve_default_template_key(
        &mut self,
        index: u32,
    ) -> Result<ExportDefaultTemplateKeyV1, Self::Error> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.records.get(index))
            .map(ExportDefaultTemplateV1::key)
            .ok_or(ExportDefaultTemplateLookupError::IndexOutOfRange {
                index,
                len: self.len,
            })
    }
}

impl ExportDefaultTemplateIndexResolver for CanonicalExportDefaultTemplatesV1 {
    type Error = ExportDefaultTemplateLookupError;

    fn resolve_default_template_index(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
    ) -> Result<u32, Self::Error> {
        self.index_of(key)
            .ok_or(ExportDefaultTemplateLookupError::MissingKey(key))
    }
}

#[derive(Debug)]
pub struct IndexedCanonicalExportDefaultTemplatesV1<'a> {
    records: Vec<IndexedExportDefaultTemplateV1<'a>>,
}

impl WireEncode for IndexedCanonicalExportDefaultTemplatesV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalExportDefaultTemplatesV1 {
    records: Vec<DecodedExportDefaultTemplateV1>,
}

impl DecodedCanonicalExportDefaultTemplatesV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalExportDefaultTemplatesV1, ExportDefaultTemplateSetValidationError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        let len = u32::try_from(self.records.len())
            .map_err(|_| ExportDefaultTemplateSetValidationError::TooMany)?;
        let mut records = Vec::<ExportDefaultTemplateV1>::with_capacity(self.records.len());
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record.resolve(resolver).map_err(|error| {
                ExportDefaultTemplateSetValidationError::Record { index, error }
            })?;
            if let Some(previous) = records.last() {
                match previous.key().cmp(&record.key()) {
                    std::cmp::Ordering::Equal => {
                        return Err(ExportDefaultTemplateSetValidationError::DuplicateKey {
                            index,
                            key: record.key(),
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(ExportDefaultTemplateSetValidationError::NonCanonicalOrder {
                            index,
                        });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            records.push(record);
        }
        Ok(CanonicalExportDefaultTemplatesV1 { records, len })
    }
}

impl WireEncode for DecodedCanonicalExportDefaultTemplatesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalExportDefaultTemplatesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExportDefaultTemplateV1::decode(decoder))
            .map(|records| Self { records })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateSetBuildError {
    TooMany,
    DuplicateKey(ExportDefaultTemplateKeyV1),
}

impl fmt::Display for ExportDefaultTemplateSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => formatter.write_str("export default-template count exceeds u32"),
            Self::DuplicateKey(key) => {
                write!(formatter, "duplicate export default template {key:?}")
            }
        }
    }
}

impl std::error::Error for ExportDefaultTemplateSetBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateLookupError {
    IndexOutOfRange { index: u32, len: u32 },
    MissingKey(ExportDefaultTemplateKeyV1),
}

impl fmt::Display for ExportDefaultTemplateLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexOutOfRange { index, len } => write!(
                formatter,
                "export default-template index {index} is out of range for length {len}"
            ),
            Self::MissingKey(key) => {
                write!(formatter, "export default template {key:?} is absent")
            }
        }
    }
}

impl std::error::Error for ExportDefaultTemplateLookupError {}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultTemplateSetIndexError {
    Record {
        index: usize,
        error: ExportDefaultTemplateIndexError,
    },
}

impl fmt::Display for ExportDefaultTemplateSetIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record { index, error } => {
                write!(
                    formatter,
                    "cannot index export default template {index}: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ExportDefaultTemplateSetIndexError {}

#[derive(Debug)]
pub enum ExportDefaultTemplateSetValidationError<E> {
    TooMany,
    Record {
        index: usize,
        error: ExportDefaultTemplateResolutionError<E>,
    },
    DuplicateKey {
        index: usize,
        key: ExportDefaultTemplateKeyV1,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl<E: fmt::Display> fmt::Display for ExportDefaultTemplateSetValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany => formatter.write_str("export default-template count exceeds u32"),
            Self::Record { index, error } => {
                write!(
                    formatter,
                    "invalid export default template {index}: {error}"
                )
            }
            Self::DuplicateKey { index, key } => {
                write!(
                    formatter,
                    "duplicate export default template {key:?} at index {index}"
                )
            }
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical export default-template order at index {index}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultTemplateSetValidationError<E>
{
}

#[cfg(test)]
mod tests;
