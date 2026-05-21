//! MIR-derived declaration metadata facts used by downstream stages.

use crate::common::FactIdentity;

/// MIR-owned metadata facts that are derived from declaration metadata roots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MirMetadataFacts {
    pub nominal_direct_supertypes: Vec<NominalDirectSupertypesFact>,
}

impl MirMetadataFacts {
    /// Return whether no MIR-derived metadata facts have been published yet.
    pub fn is_empty(&self) -> bool {
        self.nominal_direct_supertypes.is_empty()
    }

    /// Return direct supertypes for a nominal/object declaration by FQN.
    pub fn direct_supertypes(&self, fqn: &str) -> Option<&[String]> {
        self.nominal_direct_supertypes
            .iter()
            .find(|fact| fact.fqn == fqn)
            .map(|fact| fact.direct_supertypes.as_slice())
    }
}

/// Direct supertypes published for a nominal or object declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NominalDirectSupertypesFact {
    pub identity: FactIdentity,
    pub owner_kind: MirNominalOwnerKind,
    pub fqn: String,
    pub direct_supertypes: Vec<String>,
}

impl NominalDirectSupertypesFact {
    /// Create a MIR-owned nominal direct-supertype fact.
    pub fn new(
        identity: FactIdentity,
        owner_kind: MirNominalOwnerKind,
        fqn: impl Into<String>,
        direct_supertypes: Vec<String>,
    ) -> Self {
        Self {
            identity,
            owner_kind,
            fqn: fqn.into(),
            direct_supertypes,
        }
    }
}

/// Declaration kinds that can own nominal direct-supertype facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirNominalOwnerKind {
    Nominal,
    Object,
}

impl MirNominalOwnerKind {
    /// Return a stable dump/test label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Nominal => "nominal",
            Self::Object => "object",
        }
    }
}
