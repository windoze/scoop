//! Canonical diagnostic bytes derived from persistent exact-type identities.

use scoop_wire::{BudgetMeter, DecodeLimits, WireError, WirePath};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::*;
use crate::{DeclarationName, DefinitionOwnerAtom, SourceDeclarationKey, SourceDeclarationKind};

const MAX_DIAGNOSTIC_NAME_BYTES: usize = 16 * 1024 * 1024;
const MAX_DIAGNOSTIC_RECURSION: usize = 1_024;

mod atoms;
use atoms::*;
mod cost;
use cost::exact_type_cost;
mod render;
use render::write_exact_type;
mod generated;
use generated::{nominal_cost, write_nominal};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExactTypeDiagnosticName(String);

impl CanonicalExactTypeDiagnosticName {
    pub fn from_validated_graph(
        root: PersistentExactTypeId,
        graph: &impl ExactTypeDiagnosticGraph,
    ) -> Result<Self, ExactTypeDiagnosticError> {
        Self::from_validated_graph_metered(
            root,
            graph,
            &mut BudgetMeter::new(DecodeLimits::default()),
        )
    }

    pub fn from_validated_graph_metered(
        root: PersistentExactTypeId,
        graph: &impl ExactTypeDiagnosticGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactTypeDiagnosticError> {
        let mut costs = BTreeMap::new();
        let mut active = BTreeSet::new();
        let cost = exact_type_cost(root, graph, &mut costs, &mut active, 1, meter)?;
        if cost > MAX_DIAGNOSTIC_NAME_BYTES {
            return Err(ExactTypeDiagnosticError::NameTooLong {
                limit: MAX_DIAGNOSTIC_NAME_BYTES,
                observed: cost,
            });
        }
        let path = WirePath::root();
        meter.check_semantic_leaf(cost as u64, &path)?;
        meter.charge_owned_bytes(cost as u64, &path)?;
        // The render pass expands the DAG. Its work is bounded by the checked
        // output cost, rather than only by the number of unique type nodes.
        meter.charge_work((cost as u64).saturating_mul(4), &path)?;
        let mut output = String::new();
        output
            .try_reserve_exact(cost)
            .map_err(|_| ExactTypeDiagnosticError::Allocation)?;
        write_exact_type(root, graph, &mut output, 1)?;
        if output.len() != cost {
            return Err(ExactTypeDiagnosticError::GraphChangedDuringPrint);
        }
        Ok(Self(output))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CanonicalExactTypeDiagnosticName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactTypeDiagnosticError {
    Resource(WireError),
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
    RecursionLimit,
    LengthOverflow,
    NameTooLong { limit: usize, observed: usize },
    Allocation,
    GraphChangedDuringPrint,
}

impl fmt::Display for ExactTypeDiagnosticError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
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
            Self::RecursionLimit => {
                formatter.write_str("exact type diagnostic recursion limit exceeded")
            }
            Self::LengthOverflow => formatter.write_str("exact type diagnostic length overflow"),
            Self::NameTooLong { limit, observed } => {
                write!(
                    formatter,
                    "exact type diagnostic name has {observed} bytes, limit is {limit}"
                )
            }
            Self::Allocation => {
                formatter.write_str("failed to allocate exact type diagnostic name")
            }
            Self::GraphChangedDuringPrint => {
                formatter.write_str("validated exact type graph changed while printing")
            }
        }
    }
}

impl std::error::Error for ExactTypeDiagnosticError {}

impl From<WireError> for ExactTypeDiagnosticError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
