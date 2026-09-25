//! Source-side inputs for replaying GC/ZST facts from artifact bytes.
//! Resolving this transcript does not grant checked type-semantics authority.

use std::collections::BTreeSet;
use std::fmt;

use scoop_identity::{PersistentEnumVariantId, PersistentExactTypeId};
use scoop_wire::{Encoder, WireEncode, WireError, WirePath};

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
    ) -> Result<Self, TypeFactShapeSourceError> {
        records.sort_unstable_by_key(ExactTypeFactShapeRecordV1::exact);
        Self::from_ordered(records)
    }

    fn from_ordered(
        records: Vec<ExactTypeFactShapeRecordV1>,
    ) -> Result<Self, TypeFactShapeSourceError> {
        for pair in records.windows(2) {
            if pair[0].exact >= pair[1].exact {
                return Err(TypeFactShapeSourceError::NonCanonicalOrder(pair[1].exact));
            }
        }
        for record in records.iter() {
            validate_shape(&record.shape)?;
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

fn validate_shape(shape: &ExactTypeFactShapeV1) -> Result<(), TypeFactShapeSourceError> {
    match shape {
        ExactTypeFactShapeV1::Unit
        | ExactTypeFactShapeV1::Scalar
        | ExactTypeFactShapeV1::Pointer
        | ExactTypeFactShapeV1::Reference => Ok(()),
        ExactTypeFactShapeV1::OrdinaryStruct { .. }
        | ExactTypeFactShapeV1::CLayoutStruct { .. } => Ok(()),
        ExactTypeFactShapeV1::Tuple { elements } => {
            if elements.is_empty() {
                return Err(TypeFactShapeSourceError::EmptyTuple);
            }
            Ok(())
        }
        ExactTypeFactShapeV1::Enum { variants } => {
            let mut seen = BTreeSet::new();
            for variant in variants.iter() {
                if !seen.insert(variant.variant) {
                    return Err(TypeFactShapeSourceError::DuplicateVariant(variant.variant));
                }
            }
            Ok(())
        }
    }
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
