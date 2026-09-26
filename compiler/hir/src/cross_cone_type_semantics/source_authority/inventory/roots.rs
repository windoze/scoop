use super::*;
use crate::SourceNominalId;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalSourceNominalIdsV1 {
    values: Vec<SourceNominalId>,
}

impl CanonicalSourceNominalIdsV1 {
    pub fn try_new(mut values: Vec<SourceNominalId>) -> Result<Self, SourceInventoryError> {
        values.sort_unstable();
        Self::from_ordered(values)
    }

    fn from_ordered(values: Vec<SourceNominalId>) -> Result<Self, SourceInventoryError> {
        validate_order(&values, |value| *value, "source roots")?;
        Ok(Self { values })
    }

    pub fn values(&self) -> &[SourceNominalId] {
        &self.values
    }
}
