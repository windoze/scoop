use std::fmt;

use scoop_identity::{ConeIdentity, PersistentExactTypeId, SourceIdentity};
use scoop_wire::{Encoder, WireEncode};

use super::wire;
use crate::SourceNominalId;

mod decode;
mod source;
#[cfg(test)]
mod tests;

pub use decode::*;
pub use source::*;

/// Persistent access regions keep file, lexical-owner, and subclass regions
/// distinct. A source spelling or session arena id is never an access key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistentAccessConstraintV1 {
    Cone(ConeIdentity),
    File(SourceIdentity),
    LexicalOwner(SourceNominalId),
    SubclassesOf(PersistentExactTypeId),
}

impl WireEncode for PersistentAccessConstraintV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, value): (u64, &dyn WireEncode) = match self {
            Self::Cone(value) => (1, value),
            Self::File(value) => (2, value),
            Self::LexicalOwner(value) => (3, value),
            Self::SubclassesOf(value) => (4, value),
        };
        wire::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        value.encode(encoder)
    }
}

/// Canonical domain data, not a source-lookup or receiver-access witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentAccessDomainV1(Domain);

#[derive(Clone, Debug, Eq, PartialEq)]
enum Domain {
    Empty,
    Conjunction(Vec<PersistentAccessConstraintV1>),
}

impl PersistentAccessDomainV1 {
    pub const fn empty() -> Self {
        Self(Domain::Empty)
    }
    pub const fn universal() -> Self {
        Self(Domain::Conjunction(Vec::new()))
    }

    pub fn try_from_constraints(
        constraints: Vec<PersistentAccessConstraintV1>,
    ) -> Result<Self, PersistentAccessDomainError> {
        let mut keyed = constraints
            .into_iter()
            .map(|constraint| {
                scoop_wire::encode(&constraint)
                    .map(|bytes| (bytes, constraint))
                    .map_err(PersistentAccessDomainError::Encoding)
            })
            .collect::<Result<Vec<_>, _>>()?;
        keyed.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        Self::from_ordered(
            keyed
                .into_iter()
                .map(|(_, constraint)| constraint)
                .collect(),
        )
    }

    fn from_ordered(
        constraints: Vec<PersistentAccessConstraintV1>,
    ) -> Result<Self, PersistentAccessDomainError> {
        let mut previous: Option<Vec<u8>> = None;
        for (index, constraint) in constraints.iter().enumerate() {
            let bytes =
                scoop_wire::encode(constraint).map_err(PersistentAccessDomainError::Encoding)?;
            if previous.as_ref().is_some_and(|previous| previous >= &bytes) {
                return Err(PersistentAccessDomainError::NonCanonicalOrder { index });
            }
            previous = Some(bytes);
        }
        Ok(Self(Domain::Conjunction(constraints)))
    }

    pub fn constraints(&self) -> &[PersistentAccessConstraintV1] {
        match &self.0 {
            Domain::Empty => &[],
            Domain::Conjunction(constraints) => constraints,
        }
    }
    pub fn is_empty(&self) -> bool {
        matches!(self.0, Domain::Empty)
    }
    pub fn is_universal(&self) -> bool {
        matches!(&self.0, Domain::Conjunction(constraints) if constraints.is_empty())
    }

    /// Intersects already canonical domains. An overlap between their sets is
    /// one shared constraint, rather than duplicate producer input.
    pub fn intersect(&self, other: &Self) -> Result<Self, PersistentAccessDomainError> {
        if self.is_empty() || other.is_empty() {
            return Ok(Self::empty());
        }
        let mut constraints = self.constraints().to_vec();
        for constraint in other.constraints() {
            if !constraints.contains(constraint) {
                constraints.push(constraint.clone());
            }
        }
        Self::try_from_constraints(constraints)
    }
}

impl WireEncode for PersistentAccessDomainV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            Domain::Empty => wire::tag(encoder, 1, 1),
            Domain::Conjunction(constraints) => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                wire::sequence(encoder, constraints)
            }
        }
    }
}

/// Purpose-specific domains have no conversion between one another.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentLookupDomainV1(PersistentAccessDomainV1);
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentInheritanceDomainV1(PersistentAccessDomainV1);
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentSlotContractDomainV1(PersistentAccessDomainV1);

macro_rules! domain_purpose {
    ($name:ident) => {
        impl $name {
            pub const fn new(domain: PersistentAccessDomainV1) -> Self {
                Self(domain)
            }
            pub const fn domain(&self) -> &PersistentAccessDomainV1 {
                &self.0
            }
        }
        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                self.0.encode(encoder)
            }
        }
    };
}
domain_purpose!(PersistentLookupDomainV1);
domain_purpose!(PersistentInheritanceDomainV1);
domain_purpose!(PersistentSlotContractDomainV1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalAccessDomainsV1 {
    lookup: PersistentLookupDomainV1,
    inheritance: PersistentInheritanceDomainV1,
    slot: PersistentSlotContractDomainV1,
}

impl NominalAccessDomainsV1 {
    pub const fn new(
        lookup: PersistentLookupDomainV1,
        inheritance: PersistentInheritanceDomainV1,
        slot: PersistentSlotContractDomainV1,
    ) -> Self {
        Self {
            lookup,
            inheritance,
            slot,
        }
    }
    pub const fn lookup(&self) -> &PersistentLookupDomainV1 {
        &self.lookup
    }
    pub const fn inheritance(&self) -> &PersistentInheritanceDomainV1 {
        &self.inheritance
    }
    pub const fn slot(&self) -> &PersistentSlotContractDomainV1 {
        &self.slot
    }
}

impl WireEncode for NominalAccessDomainsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.lookup.encode(encoder)?;
        encoder.field(2)?;
        self.inheritance.encode(encoder)?;
        encoder.field(3)?;
        self.slot.encode(encoder)
    }
}

#[derive(Debug)]
pub enum PersistentAccessDomainError {
    Encoding(scoop_wire::cbor::EncodeError),
    NonCanonicalOrder { index: usize },
}

impl fmt::Display for PersistentAccessDomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoding(error) => {
                write!(f, "cannot encode persistent access constraint: {error}")
            }
            Self::NonCanonicalOrder { index } => write!(
                f,
                "duplicate or noncanonical persistent access constraint at index {index}"
            ),
        }
    }
}
impl std::error::Error for PersistentAccessDomainError {}
