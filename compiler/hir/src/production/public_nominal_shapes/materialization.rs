//! Transient representation/inheritance closure over the shared declarations.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::PersistentTypeId;
use scoop_wire::{WireError, WirePath};

use crate::{CanonicalCallableInterfacesV1, CanonicalNominalInterfacesV1, SourceNominalId};

mod declarations;
mod requirements;
pub use requirements::NominalMaterializationRequirementV1;

/// This is a query result, not a serialized inventory or a machine-use permit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalMaterializationClosure {
    sources: Vec<PersistentTypeId>,
}

impl NominalMaterializationClosure {
    pub fn from_declarations(
        nominals: &CanonicalNominalInterfacesV1,
        callables: &CanonicalCallableInterfacesV1,
    ) -> Result<Self, NominalMaterializationClosureError> {
        let mut graph = Graph::new(nominals)?;
        nominals.visit_materialization_requirements(callables, |requirement| {
            graph.requirement(requirement)
        })?;
        graph.propagate()?;
        let mut sources = Vec::new();
        scoop_wire::allocation::try_reserve(
            &mut sources,
            graph.positions.len(),
            &WirePath::root(),
        )?;
        for (source, position) in graph.positions {
            if !graph.blocked[position] {
                sources.push(source);
            }
        }
        Ok(Self { sources })
    }

    pub fn contains(&self, source: PersistentTypeId) -> bool {
        self.sources.binary_search(&source).is_ok()
    }

    pub fn sources(&self) -> &[PersistentTypeId] {
        &self.sources
    }
}

struct Graph {
    positions: BTreeMap<PersistentTypeId, usize>,
    dependents: Vec<BTreeSet<usize>>,
    blocked: Vec<bool>,
    pending: Vec<usize>,
}

impl Graph {
    fn new(
        nominals: &CanonicalNominalInterfacesV1,
    ) -> Result<Self, NominalMaterializationClosureError> {
        let path = WirePath::root();

        let mut positions = BTreeMap::new();
        for record in nominals.all_records() {
            if let SourceNominalId::Concrete(source) = record.declaration() {
                let position = positions.len();
                positions.insert(source, position);
            }
        }
        let mut dependents = Vec::new();
        let mut blocked = Vec::new();
        scoop_wire::allocation::try_reserve(&mut dependents, positions.len(), &path)?;
        scoop_wire::allocation::try_reserve(&mut blocked, positions.len(), &path)?;
        dependents.resize_with(positions.len(), BTreeSet::new);
        blocked.resize(positions.len(), false);
        Ok(Self {
            positions,
            dependents,
            blocked,
            pending: Vec::new(),
        })
    }

    fn position(&self, source: SourceNominalId) -> Option<usize> {
        match source {
            SourceNominalId::Concrete(source) => self.positions.get(&source).copied(),
            SourceNominalId::GenericTemplate(_) => None,
        }
    }

    fn edge(&mut self, dependency: usize, owner: usize) -> Result<(), WireError> {
        let dependents = &mut self.dependents[dependency];

        if !dependents.contains(&owner) {
            dependents.insert(owner);
        }
        Ok(())
    }

    fn block(&mut self, owner: usize) -> Result<(), WireError> {
        if !self.blocked[owner] {
            scoop_wire::allocation::try_reserve(&mut self.pending, 1, &WirePath::root())?;
            self.blocked[owner] = true;
            self.pending.push(owner);
        }
        Ok(())
    }

    fn propagate(&mut self) -> Result<(), WireError> {
        while let Some(owner) = self.pending.pop() {
            for dependent in std::mem::take(&mut self.dependents[owner]) {
                self.block(dependent)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalMaterializationClosureError {
    MissingNominal(PersistentTypeId),
    Resource(WireError),
}

impl From<WireError> for NominalMaterializationClosureError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for NominalMaterializationClosureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid shared nominal materialization closure: {self:?}"
        )
    }
}

impl std::error::Error for NominalMaterializationClosureError {}
