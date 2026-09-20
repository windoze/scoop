//! Source-side inputs for replaying GC/ZST facts from artifact bytes.
//! Resolving this transcript does not grant checked type-semantics authority.

use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{PersistentEnumVariantId, PersistentExactTypeId};
use scoop_wire::{BudgetMeter, Encoder, WireEncode, WireError, WirePath};

use super::{ExactEnumVariantFactsV1, ExactTypeFactShapeV1, wire};

mod decode;
mod encode;
pub use decode::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactTypeFactShapeRecordV1 {
    exact: PersistentExactTypeId,
    shape: ExactTypeFactShapeV1,
}

impl ExactTypeFactShapeRecordV1 {
    pub const fn new(exact: PersistentExactTypeId, shape: ExactTypeFactShapeV1) -> Self {
        Self { exact, shape }
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }

    pub const fn shape(&self) -> &ExactTypeFactShapeV1 {
        &self.shape
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExactTypeFactShapesV1 {
    records: Vec<ExactTypeFactShapeRecordV1>,
}

impl CanonicalExactTypeFactShapesV1 {
    pub fn try_new(
        mut records: Vec<ExactTypeFactShapeRecordV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, TypeFactShapeSourceError> {
        let path = WirePath::root();
        let count = records.len() as u64;
        meter.check_table_entries(count, &path)?;
        meter.charge_work(
            count.saturating_mul(u64::from(count.max(1).ilog2()) + 1),
            &path,
        )?;
        records.sort_unstable_by_key(ExactTypeFactShapeRecordV1::exact);
        Self::from_ordered(records, meter)
    }

    fn from_ordered(
        records: Vec<ExactTypeFactShapeRecordV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, TypeFactShapeSourceError> {
        let path = WirePath::root();
        meter.check_table_entries(records.len() as u64, &path)?;
        meter.charge_nodes(records.len() as u64, &path)?;
        meter.charge_work(records.len() as u64, &path)?;
        for pair in records.windows(2) {
            if pair[0].exact >= pair[1].exact {
                return Err(TypeFactShapeSourceError::NonCanonicalOrder(pair[1].exact));
            }
        }
        for (index, record) in records.iter().enumerate() {
            validate_shape(&record.shape, meter, &path.clone().index(index as u64))?;
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[ExactTypeFactShapeRecordV1] {
        &self.records
    }

    pub fn get(&self, exact: PersistentExactTypeId) -> Option<&ExactTypeFactShapeV1> {
        self.records
            .binary_search_by_key(&exact, ExactTypeFactShapeRecordV1::exact)
            .ok()
            .map(|index| &self.records[index].shape)
    }
}

fn validate_shape(
    shape: &ExactTypeFactShapeV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeFactShapeSourceError> {
    meter.check_semantic_depth(1, path)?;
    meter.charge_work(1, path)?;
    match shape {
        ExactTypeFactShapeV1::Unit
        | ExactTypeFactShapeV1::Scalar
        | ExactTypeFactShapeV1::Pointer
        | ExactTypeFactShapeV1::Reference => Ok(()),
        ExactTypeFactShapeV1::OrdinaryStruct { fields }
        | ExactTypeFactShapeV1::CLayoutStruct { fields } => check_fields(fields.len(), meter, path),
        ExactTypeFactShapeV1::Tuple { elements } => {
            if elements.is_empty() {
                return Err(TypeFactShapeSourceError::EmptyTuple);
            }
            check_fields(elements.len(), meter, path)
        }
        ExactTypeFactShapeV1::Enum { variants } => {
            meter.check_table_entries(variants.len() as u64, path)?;
            meter.charge_nodes(variants.len() as u64, path)?;
            let mut seen = BTreeSet::new();
            for (index, variant) in variants.iter().enumerate() {
                let path = path.clone().index(index as u64);
                meter.check_semantic_depth(2, &path)?;
                meter.charge_work(u64::from((index + 1).ilog2()) + 1, &path)?;
                if !seen.insert(variant.variant) {
                    return Err(TypeFactShapeSourceError::DuplicateVariant(variant.variant));
                }
                check_fields(variant.fields.len(), meter, &path)?;
            }
            Ok(())
        }
    }
}

fn check_fields(
    count: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeFactShapeSourceError> {
    meter.check_table_entries(count as u64, path)?;
    meter.charge_work(count as u64, path)?;
    Ok(())
}

#[derive(Debug)]
pub enum TypeFactShapeSourceError {
    Resource(WireError),
    Reference(String),
    NonCanonicalOrder(PersistentExactTypeId),
    DuplicateVariant(PersistentEnumVariantId),
    EmptyTuple,
}

impl From<WireError> for TypeFactShapeSourceError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for TypeFactShapeSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Reference(error) => write!(f, "invalid source fact-shape identity: {error}"),
            Self::NonCanonicalOrder(exact) => write!(
                f,
                "duplicate or non-canonical source fact-shape order at {exact}"
            ),
            Self::DuplicateVariant(variant) => {
                write!(f, "duplicate source fact-shape variant {variant}")
            }
            Self::EmptyTuple => {
                f.write_str("source fact-shape tuple must have at least one element")
            }
        }
    }
}

impl std::error::Error for TypeFactShapeSourceError {}

#[cfg(test)]
mod tests;
