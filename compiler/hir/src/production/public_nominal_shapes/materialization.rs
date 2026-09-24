//! Transient representation/inheritance closure over the shared declarations.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::PersistentTypeId;
use scoop_wire::{BudgetMeter, WireError, WirePath};

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
        meter: &mut BudgetMeter,
    ) -> Result<Self, NominalMaterializationClosureError> {
        let mut graph = Graph::new(nominals, meter)?;
        nominals.visit_materialization_requirements(callables, meter, |requirement, meter| {
            graph.requirement(requirement, meter)
        })?;
        graph.propagate(meter)?;
        let mut sources = Vec::new();
        meter.try_reserve_collection_slots(
            &mut sources,
            graph.positions.len(),
            &WirePath::root(),
        )?;
        for (source, position) in graph.positions {
            meter.charge_work(1, &WirePath::root())?;
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
        meter: &mut BudgetMeter,
    ) -> Result<Self, NominalMaterializationClosureError> {
        let path = WirePath::root();
        let count = nominals.declaration_count();
        meter.check_table_entries(count as u64, &path)?;
        let mut positions = BTreeMap::new();
        for record in nominals.all_records() {
            meter.charge_work(1 + u64::from(positions.len().max(1).ilog2()), &path)?;
            if let SourceNominalId::Concrete(source) = record.declaration() {
                meter.charge_collection_slots(1, &path)?;
                let position = positions.len();
                positions.insert(source, position);
            }
        }
        let mut dependents = Vec::new();
        let mut blocked = Vec::new();
        meter.try_reserve_collection_slots(&mut dependents, positions.len(), &path)?;
        meter.try_reserve_collection_slots(&mut blocked, positions.len(), &path)?;
        dependents.resize_with(positions.len(), BTreeSet::new);
        blocked.resize(positions.len(), false);
        Ok(Self {
            positions,
            dependents,
            blocked,
            pending: Vec::new(),
        })
    }

    fn position(
        &self,
        source: SourceNominalId,
        meter: &mut BudgetMeter,
    ) -> Result<Option<usize>, WireError> {
        meter.charge_work(
            1 + u64::from(self.positions.len().max(1).ilog2()),
            &WirePath::root(),
        )?;
        Ok(match source {
            SourceNominalId::Concrete(source) => self.positions.get(&source).copied(),
            SourceNominalId::GenericTemplate(_) => None,
        })
    }

    fn edge(
        &mut self,
        dependency: usize,
        owner: usize,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        let path = WirePath::root();
        let dependents = &mut self.dependents[dependency];
        meter.charge_work(1 + u64::from(dependents.len().max(1).ilog2()), &path)?;
        if !dependents.contains(&owner) {
            meter.check_table_entries(dependents.len() as u64 + 1, &path)?;
            meter.charge_edges(1, &path)?;
            meter.charge_collection_slots(1, &path)?;
            dependents.insert(owner);
        }
        Ok(())
    }

    fn block(&mut self, owner: usize, meter: &mut BudgetMeter) -> Result<(), WireError> {
        if !self.blocked[owner] {
            meter.try_reserve_collection_slots(&mut self.pending, 1, &WirePath::root())?;
            self.blocked[owner] = true;
            self.pending.push(owner);
        }
        Ok(())
    }

    fn propagate(&mut self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        while let Some(owner) = self.pending.pop() {
            for dependent in std::mem::take(&mut self.dependents[owner]) {
                meter.charge_work(1, &WirePath::root())?;
                self.block(dependent, meter)?;
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
