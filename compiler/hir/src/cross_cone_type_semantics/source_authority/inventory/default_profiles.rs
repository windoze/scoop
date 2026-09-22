use super::*;
use crate::{
    CanonicalDefaultSourceTemplatesV1, ProtectedDefaultTemplateKeyV1,
    ProtectedDefaultWitnessSourceProfileV1,
};

mod wire;
pub use wire::DecodedCanonicalDefaultSourceProfilesV1;

/// Source classification only; artifact binding and access replay remain required.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefaultSourceProfileV1 {
    key: ProtectedDefaultTemplateKeyV1,
    profile: ProtectedDefaultWitnessSourceProfileV1,
}
impl DefaultSourceProfileV1 {
    pub const fn new(
        key: ProtectedDefaultTemplateKeyV1,
        profile: ProtectedDefaultWitnessSourceProfileV1,
    ) -> Self {
        Self { key, profile }
    }
    pub const fn key(&self) -> ProtectedDefaultTemplateKeyV1 {
        self.key
    }
    pub const fn profile(&self) -> ProtectedDefaultWitnessSourceProfileV1 {
        self.profile
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalDefaultSourceProfilesV1 {
    records: Vec<DefaultSourceProfileV1>,
}
impl CanonicalDefaultSourceProfilesV1 {
    pub fn try_new(
        mut records: Vec<DefaultSourceProfileV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        charge_sort(records.len(), meter)?;
        records.sort_unstable_by_key(DefaultSourceProfileV1::key);
        Self::from_ordered(records, meter)
    }
    fn from_ordered(
        records: Vec<DefaultSourceProfileV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, SourceInventoryError> {
        meter.check_semantic_depth(1, &WirePath::root())?;
        meter.charge_nodes(1, &WirePath::root())?;
        meter.charge_work(1, &WirePath::root())?;
        validate_order(
            &records,
            DefaultSourceProfileV1::key,
            "default profiles",
            meter,
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[DefaultSourceProfileV1] {
        &self.records
    }
    pub fn get(&self, key: ProtectedDefaultTemplateKeyV1) -> Option<&DefaultSourceProfileV1> {
        self.records
            .binary_search_by_key(&key, DefaultSourceProfileV1::key)
            .ok()
            .map(|i| &self.records[i])
    }
    pub fn validate_template_coverage(
        &self,
        templates: &CanonicalDefaultSourceTemplatesV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), SourceInventoryError> {
        let path = WirePath::root();
        meter.check_semantic_depth(1, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.check_table_entries(templates.records().len() as u64, &path)?;
        meter.charge_work(
            self.records.len() as u64 + templates.records().len() as u64 + 1,
            &path,
        )?;
        let mut actual = self.records.iter();
        for template in templates.records() {
            let expected = template.key();
            let Some(record) = actual.next() else {
                return Err(SourceInventoryError::MissingDefaultProfile(expected));
            };
            match record.key().cmp(&expected) {
                std::cmp::Ordering::Less => {
                    return Err(SourceInventoryError::UnexpectedDefaultProfile(record.key()));
                }
                std::cmp::Ordering::Greater => {
                    return Err(SourceInventoryError::MissingDefaultProfile(expected));
                }
                std::cmp::Ordering::Equal => {}
            }
        }
        if let Some(record) = actual.next() {
            return Err(SourceInventoryError::UnexpectedDefaultProfile(record.key()));
        }
        Ok(())
    }
}
