//! Portable source regions. These describe declarations, never machine types.

use scoop_identity::{ConeIdentity, SourceIdentity};
use scoop_wire::WireError;

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
    ) -> Result<Self, WireError> {
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
}

#[cfg(test)]
mod tests;
