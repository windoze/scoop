//! Portable source regions. These describe declarations, never machine types.

use scoop_identity::{ConeIdentity, SourceIdentity};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use crate::SourceNominalId;

mod wire;
pub use wire::{
    DecodedSourceAccessConstraintV1, DecodedSourceAccessDomainV1,
    SourceAccessDomainResolutionError, SourceAccessDomainResolver,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceAccessConstraintV1 {
    Cone(ConeIdentity),
    File(SourceIdentity),
    LexicalOwner(SourceNominalId),
    SubclassesOf(SourceNominalId),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceAccessDomainV1(Domain);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Domain {
    Empty,
    Conjunction(Vec<SourceAccessConstraintV1>),
}

impl SourceAccessDomainV1 {
    pub const fn empty() -> Self {
        Self(Domain::Empty)
    }

    pub const fn universal() -> Self {
        Self(Domain::Conjunction(Vec::new()))
    }

    /// Canonicalizes a producer's conjunction, including repeated owner regions.
    pub fn from_constraints(
        mut constraints: Vec<SourceAccessConstraintV1>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, WireError> {
        Self::charge_constraints(&constraints, meter, path)?;
        let comparisons = u64::from(constraints.len().max(1).ilog2()) + 1;
        meter.charge_work(comparisons.saturating_mul(constraints.len() as u64), path)?;
        constraints.sort_unstable();
        constraints.dedup();
        Ok(Self(Domain::Conjunction(constraints)))
    }

    pub const fn is_empty(&self) -> bool {
        matches!(self.0, Domain::Empty)
    }

    pub fn is_universal(&self) -> bool {
        matches!(&self.0, Domain::Conjunction(constraints) if constraints.is_empty())
    }

    pub fn constraints(&self) -> &[SourceAccessConstraintV1] {
        match &self.0 {
            Domain::Empty => &[],
            Domain::Conjunction(constraints) => constraints,
        }
    }

    fn charge_constraints(
        constraints: &[SourceAccessConstraintV1],
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), WireError> {
        meter.check_table_entries(constraints.len() as u64, path)?;
        meter.charge_nodes(constraints.len() as u64 + 1, path)?;
        meter.charge_work(constraints.len() as u64 + 1, path)?;
        for constraint in constraints {
            if let SourceAccessConstraintV1::File(source) = constraint {
                let bytes = source.logical_path().as_str().len() as u64;
                meter.check_semantic_leaf(bytes, path)?;
                meter.charge_work(
                    bytes.saturating_mul(u64::from(constraints.len().max(1).ilog2()) + 1),
                    path,
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
