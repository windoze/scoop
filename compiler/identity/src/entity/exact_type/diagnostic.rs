//! Canonical diagnostic bytes derived from persistent exact-type identities.

use scoop_wire::HashError;
use std::collections::BTreeSet;
use std::fmt;

use super::*;
use crate::{DeclarationName, DefinitionOwnerAtom, SourceDeclarationKey, SourceDeclarationKind};

mod atoms;
use atoms::*;
mod render;
use render::write_exact_type;
mod generated;
use generated::write_nominal;

/// Read-only access to an already validated exact-type identity graph.
pub trait ExactTypeDiagnosticGraph {
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Option<&ExactTypeKey>;

    fn source_type_declaration(&self, id: PersistentTypeId) -> Option<&SourceDeclarationKey>;

    fn generated_nominal_key(&self, id: PersistentTypeId) -> Option<&crate::GeneratedNominalKey>;

    fn source_generic_type_declaration(
        &self,
        id: PersistentGenericTypeId,
    ) -> Option<&SourceDeclarationKey>;

    fn cone_coordinate(&self, id: ConeIdentity) -> Option<&ConeCoordinate>;
}

/// A checked read-only view over canonical identities and the manifest
/// coordinates that name their defining Cones.
pub struct ExactTypeDiagnosticCatalog<'a> {
    identities: &'a crate::ValidatedIdentityGraph,
    coordinates: Vec<(ConeIdentity, &'a ConeCoordinate)>,
}

impl<'a> ExactTypeDiagnosticCatalog<'a> {
    pub fn try_new(
        identities: &'a crate::ValidatedIdentityGraph,
        coordinates: &'a [ConeCoordinate],
    ) -> Result<Self, ExactTypeDiagnosticCatalogError> {
        let mut indexed = Vec::new();
        indexed
            .try_reserve_exact(coordinates.len())
            .map_err(|_| ExactTypeDiagnosticCatalogError::Allocation)?;
        for coordinate in coordinates {
            let id = coordinate
                .identity()
                .map_err(ExactTypeDiagnosticCatalogError::Hash)?;
            if !identities.contains_resolved_identity(id) {
                return Err(ExactTypeDiagnosticCatalogError::UnknownCone(id));
            }
            indexed.push((id, coordinate));
        }
        indexed.sort_unstable_by_key(|(id, _)| *id);
        if let Some(pair) = indexed.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(ExactTypeDiagnosticCatalogError::DuplicateCone(pair[0].0));
        }
        Ok(Self {
            identities,
            coordinates: indexed,
        })
    }
}

impl ExactTypeDiagnosticGraph for ExactTypeDiagnosticCatalog<'_> {
    fn exact_type_key(&self, id: PersistentExactTypeId) -> Option<&ExactTypeKey> {
        self.identities.canonical_key_ref(id)
    }

    fn source_type_declaration(&self, id: PersistentTypeId) -> Option<&SourceDeclarationKey> {
        self.identities.canonical_key_ref(id)
    }

    fn generated_nominal_key(&self, id: PersistentTypeId) -> Option<&crate::GeneratedNominalKey> {
        self.identities.canonical_key_ref(id)
    }

    fn source_generic_type_declaration(
        &self,
        id: PersistentGenericTypeId,
    ) -> Option<&SourceDeclarationKey> {
        self.identities.canonical_key_ref(id)
    }

    fn cone_coordinate(&self, id: ConeIdentity) -> Option<&ConeCoordinate> {
        self.coordinates
            .binary_search_by_key(&id, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.coordinates[index].1)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactTypeDiagnosticCatalogError {
    Allocation,
    UnknownCone(ConeIdentity),
    DuplicateCone(ConeIdentity),
    Hash(HashError),
}

impl fmt::Display for ExactTypeDiagnosticCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid exact-type diagnostic catalog: {self:?}")
    }
}

impl std::error::Error for ExactTypeDiagnosticCatalogError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExactTypeDiagnosticName(String);

impl CanonicalExactTypeDiagnosticName {
    pub fn from_validated_graph(
        root: PersistentExactTypeId,
        graph: &impl ExactTypeDiagnosticGraph,
    ) -> Result<Self, ExactTypeDiagnosticError> {
        let mut output = NameOutput(String::new());
        write_exact_type(root, graph, &mut output)?;
        Ok(Self(output.0))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for CanonicalExactTypeDiagnosticName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactTypeDiagnosticError {
    ConflictingNominalDefinitions(PersistentTypeId),
    InvalidGeneratedNominal(PersistentTypeId),
    MissingExactType(PersistentExactTypeId),
    MissingSourceType(PersistentTypeId),
    MissingSourceGenericType(PersistentGenericTypeId),
    MissingCone(ConeIdentity),
    NonNominalDeclaration,
    ConstructorUsedAsNominalName,
    NonNominalOwner,
    Cycle(PersistentExactTypeId),
    LengthOverflow,
    Allocation,
}

impl fmt::Display for ExactTypeDiagnosticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConflictingNominalDefinitions(id) => write!(
                formatter,
                "both source and generated nominal definitions for {id}"
            ),
            Self::InvalidGeneratedNominal(id) => {
                write!(formatter, "generated nominal key does not define {id}")
            }
            Self::MissingExactType(id) => write!(formatter, "missing exact type {id}"),
            Self::MissingSourceType(id) => write!(formatter, "missing source type {id}"),
            Self::MissingSourceGenericType(id) => {
                write!(formatter, "missing source generic type {id}")
            }
            Self::MissingCone(id) => write!(formatter, "missing Cone coordinate for {id}"),
            Self::NonNominalDeclaration => {
                formatter.write_str("exact nominal type refers to a non-nominal declaration")
            }
            Self::ConstructorUsedAsNominalName => {
                formatter.write_str("nominal declaration has a constructor name")
            }
            Self::NonNominalOwner => {
                formatter.write_str("nominal declaration has a non-nominal owner")
            }
            Self::Cycle(id) => write!(formatter, "cycle in exact type graph at {id}"),
            Self::LengthOverflow => formatter.write_str("exact type diagnostic length overflow"),
            Self::Allocation => {
                formatter.write_str("failed to allocate exact type diagnostic name")
            }
        }
    }
}

impl std::error::Error for ExactTypeDiagnosticError {}

struct NameOutput(String);

impl NameOutput {
    fn push_str(&mut self, value: &str) -> Result<(), ExactTypeDiagnosticError> {
        self.0
            .len()
            .checked_add(value.len())
            .ok_or(ExactTypeDiagnosticError::LengthOverflow)?;
        self.0
            .try_reserve(value.len())
            .map_err(|_| ExactTypeDiagnosticError::Allocation)?;
        self.0.push_str(value);
        Ok(())
    }

    fn push(&mut self, value: char) -> Result<(), ExactTypeDiagnosticError> {
        self.push_str(value.encode_utf8(&mut [0; 4]))
    }
}
