//! Canonical independent source bodies, with exact parameter-omission coverage.
use crate::{DefaultSourceTemplateV1, ProtectedDefaultTemplateKeyV1};
use scoop_wire::WirePath;
mod coverage;
mod errors;
mod indexed;
mod origins;
mod wire;
pub use coverage::*;
pub use errors::*;
pub use indexed::*;
pub use wire::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalDefaultSourceTemplatesV1 {
    records: Vec<DefaultSourceTemplateV1>,
}
impl CanonicalDefaultSourceTemplatesV1 {
    pub fn try_new(
        mut records: Vec<DefaultSourceTemplateV1>,
    ) -> Result<Self, DefaultSourceTemplateTableBuildError> {
        use DefaultSourceTemplateTableBuildError as Error;

        u32::try_from(records.len()).map_err(|_| Error::TooMany)?;

        records.sort_unstable_by_key(DefaultSourceTemplateV1::key);
        Self::from_ordered(records)
    }
    fn from_ordered(
        records: Vec<DefaultSourceTemplateV1>,
    ) -> Result<Self, DefaultSourceTemplateTableBuildError> {
        use DefaultSourceTemplateTableBuildError as Error;
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
        Ok(Self { records })
    }
    pub fn records(&self) -> &[DefaultSourceTemplateV1] {
        &self.records
    }
    pub fn get(&self, key: ProtectedDefaultTemplateKeyV1) -> Option<&DefaultSourceTemplateV1> {
        self.records
            .binary_search_by_key(&key, DefaultSourceTemplateV1::key)
            .ok()
            .map(|index| &self.records[index])
    }
}
