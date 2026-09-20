//! Canonical independent source bodies, with exact parameter-omission coverage.
use crate::{DefaultSourceTemplateV1, ProtectedDefaultTemplateKeyV1};
use scoop_wire::{BudgetMeter, WirePath};
mod coverage;
mod errors;
mod indexed;
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
        meter: &mut BudgetMeter,
    ) -> Result<Self, DefaultSourceTemplateTableBuildError> {
        use DefaultSourceTemplateTableBuildError as Error;
        let count = records.len() as u64;
        let path = WirePath::root();
        u32::try_from(records.len()).map_err(|_| Error::TooMany)?;
        meter
            .check_semantic_depth(1, &path)
            .map_err(Error::Resource)?;
        meter
            .charge_collection_slots(count, &path)
            .map_err(Error::Resource)?;
        meter
            .charge_work(
                count
                    .saturating_mul(u64::from(count.max(1).ilog2()) + 1)
                    .saturating_mul(64),
                &path,
            )
            .map_err(Error::Resource)?;
        records.sort_unstable_by_key(DefaultSourceTemplateV1::key);
        Self::from_ordered(records, meter)
    }
    fn from_ordered(
        records: Vec<DefaultSourceTemplateV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, DefaultSourceTemplateTableBuildError> {
        use DefaultSourceTemplateTableBuildError as Error;
        u32::try_from(records.len()).map_err(|_| Error::TooMany)?;
        meter
            .check_semantic_depth(1, &WirePath::root())
            .map_err(Error::Resource)?;
        meter
            .check_table_entries(records.len() as u64, &WirePath::root())
            .map_err(Error::Resource)?;
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(Error::Resource)?;
        meter
            .charge_work(
                (records.len() as u64).saturating_mul(64).saturating_add(1),
                &WirePath::root(),
            )
            .map_err(Error::Resource)?;
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
