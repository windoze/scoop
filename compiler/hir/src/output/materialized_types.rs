//! A transient query over actual LocalConcrete HIR, never a use-site permit.

use std::collections::BTreeSet;

use scoop_wire::{BudgetMeter, WireError, WirePath};

use crate::{LocalConcreteHirOutput, concrete::*};

mod expressions;
mod roots;
mod shapes;

impl LocalConcreteHirOutput {
    /// Replays type requirements from materialized source roots and their
    /// semantic children. The complete identity arena is only a lookup table;
    /// unrelated source metadata and intrinsic declarations are not roots.
    pub fn materialized_type_closure(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<TypeId>, MaterializedTypeClosureError> {
        let mut collector = Collector {
            module: self.module(),
            seen: BTreeSet::new(),
            pending: Vec::new(),
        };
        collector.roots(self, meter)?;
        self.module()
            .visit_executable_expressions(meter, |occurrence, meter| {
                collector.expression(occurrence.expression, meter)
            })
            .map_err(|error| match error {
                ExecutableExpressionVisitError::Structure(error) => {
                    MaterializedTypeClosureError::Executable(error)
                }
                ExecutableExpressionVisitError::Visitor(error) => error,
            })?;
        while let Some((ty, depth)) = collector.pending.pop() {
            collector.children(ty, depth, meter)?;
        }
        let mut result = Vec::new();
        meter.charge_work(collector.seen.len() as u64, &WirePath::root())?;
        meter.try_reserve_collection_slots(&mut result, collector.seen.len(), &WirePath::root())?;
        result.extend(collector.seen);
        Ok(result)
    }
}

struct Collector<'a> {
    module: &'a Module,
    seen: BTreeSet<TypeId>,
    pending: Vec<(TypeId, u64)>,
}

impl Collector<'_> {
    fn add(
        &mut self,
        ty: TypeId,
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<(), MaterializedTypeClosureError> {
        let path = WirePath::root();
        meter.check_semantic_depth(depth, &path)?;
        meter.charge_work(1 + u64::from(self.seen.len().max(1).ilog2()), &path)?;
        meter.charge_edges(1, &path)?;
        if ty.into_raw().into_u32() as usize >= self.module.types.len() {
            return Err(MaterializedTypeClosureError::MissingType(ty));
        }
        if !self.seen.contains(&ty) {
            meter.check_table_entries(self.seen.len() as u64 + 1, &path)?;
            meter.charge_nodes(1, &path)?;
            meter.charge_collection_slots(1, &path)?;
            meter.charge_owned_bytes(
                std::mem::size_of::<TypeId>() as u64 + std::mem::size_of::<(TypeId, u64)>() as u64,
                &path,
            )?;
            meter.try_reserve_collection_slots(&mut self.pending, 1, &path)?;
            self.seen.insert(ty);
            self.pending.push((ty, depth));
        }
        Ok(())
    }

    fn types(
        &mut self,
        types: impl IntoIterator<Item = TypeId>,
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<(), MaterializedTypeClosureError> {
        for ty in types {
            self.add(ty, depth, meter)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaterializedTypeClosureError {
    Resource(WireError),
    Executable(ExecutableExpressionStructureError),
    MissingType(TypeId),
    DepthOverflow,
}

impl From<WireError> for MaterializedTypeClosureError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for MaterializedTypeClosureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid materialized HIR type closure: {self:?}")
    }
}

impl std::error::Error for MaterializedTypeClosureError {}
