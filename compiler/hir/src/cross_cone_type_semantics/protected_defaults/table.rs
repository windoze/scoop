use super::{
    IndexedProtectedDefaultTemplateV1, ProtectedDefaultTemplateIndexError,
    ProtectedDefaultTemplateV1,
};
use crate::{ProtectedDefaultKeyIndexV1, ProtectedDefaultTemplateKeyV1};
use scoop_wire::{Encoder, WireEncode};

mod decode;
mod errors;
mod semantics;
#[cfg(test)]
mod tests;
pub use decode::*;
pub use errors::*;
pub use semantics::*;

/// Complete template data and its exact protected-protocol key projection.
/// Source/body/access validation is required before any template can be used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalProtectedDefaultTemplatesV1 {
    records: Vec<ProtectedDefaultTemplateV1>,
    keys: ProtectedDefaultKeyIndexV1,
}
impl CanonicalProtectedDefaultTemplatesV1 {
    pub fn try_new(
        mut records: Vec<ProtectedDefaultTemplateV1>,
    ) -> Result<Self, ProtectedDefaultTemplateTableBuildError> {
        records.sort_unstable_by_key(ProtectedDefaultTemplateV1::key);
        Self::from_ordered(records)
    }
    fn from_ordered(
        records: Vec<ProtectedDefaultTemplateV1>,
    ) -> Result<Self, ProtectedDefaultTemplateTableBuildError> {
        use ProtectedDefaultTemplateTableBuildError as Error;
        u32::try_from(records.len()).map_err(|_| Error::TooMany)?;
        for (offset, pair) in records.windows(2).enumerate() {
            match pair[0].key().cmp(&pair[1].key()) {
                std::cmp::Ordering::Equal => return Err(Error::Duplicate(pair[1].key())),
                std::cmp::Ordering::Greater => {
                    return Err(Error::NonCanonicalOrder { index: offset + 1 });
                }
                std::cmp::Ordering::Less => {}
            }
        }
        // The order above is checked before deriving this auxiliary index.
        let keys = ProtectedDefaultKeyIndexV1::try_new(
            records
                .iter()
                .map(ProtectedDefaultTemplateV1::key)
                .collect(),
        )
        .map_err(Error::KeyIndex)?;
        Ok(Self { records, keys })
    }
    pub fn records(&self) -> &[ProtectedDefaultTemplateV1] {
        &self.records
    }
    pub const fn keys(&self) -> &ProtectedDefaultKeyIndexV1 {
        &self.keys
    }
    pub fn get(&self, key: ProtectedDefaultTemplateKeyV1) -> Option<&ProtectedDefaultTemplateV1> {
        self.keys
            .index(key)
            .ok()
            .and_then(|index| self.records.get(index.get() as usize))
    }
    pub fn index_locals(
        &self,
    ) -> Result<
        IndexedCanonicalProtectedDefaultTemplatesV1<'_>,
        ProtectedDefaultTemplateTableIndexError,
    > {
        let mut records = Vec::with_capacity(self.records.len());
        for (index, record) in self.records.iter().enumerate() {
            records.push(
                record
                    .index_locals()
                    .map_err(|error| ProtectedDefaultTemplateTableIndexError { index, error })?,
            );
        }
        Ok(IndexedCanonicalProtectedDefaultTemplatesV1 { records })
    }
}

#[derive(Debug)]
pub struct IndexedCanonicalProtectedDefaultTemplatesV1<'a> {
    records: Vec<IndexedProtectedDefaultTemplateV1<'a>>,
}
impl WireEncode for IndexedCanonicalProtectedDefaultTemplatesV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ProtectedDefaultTemplateTableIndexError {
    pub index: usize,
    pub error: ProtectedDefaultTemplateIndexError,
}
impl std::fmt::Display for ProtectedDefaultTemplateTableIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid protected default template local indices at {}: {}",
            self.index, self.error
        )
    }
}
impl std::error::Error for ProtectedDefaultTemplateTableIndexError {}
