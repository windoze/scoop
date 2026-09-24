//! Dispatch replay from shared HIR schemas, declarations and actual selections.

use scoop_hir as hir;
use scoop_identity::{
    DispatchDeclarationOwner, GeneratedCallableKey, PersistentDispatchSlotId,
    PersistentExactTypeId, PersistentGeneratedCallableId, StrongCallableDefinitionOwner,
};
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::{BTreeMap, BTreeSet};

mod abstract_targets;
mod bindings;
mod entries;
mod errors;
mod inventory;
use SharedMirDispatchComponent as Component;
use SharedMirDispatchValidationError as Error;
pub use errors::{SharedMirDispatchComponent, SharedMirDispatchValidationError};

pub fn validate_shared_mir_dispatch(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[hir::CheckedSharedTypeFoundationV1<'_>],
    callables: &mir::CanonicalMirCallableBindingsV1,
    dependency_callables: &[&mir::CanonicalMirCallableBindingsV1],
    dispatch: &mir::CanonicalMirDispatchSchemasV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut tables = Vec::new();
    meter.try_reserve_collection_slots(
        &mut tables,
        dependency_callables.len() + 1,
        &WirePath::root(),
    )?;
    tables.push(callables);
    tables.extend_from_slice(dependency_callables);
    let index = mir::MirTypeBridgeCallableIndexV1::try_new(&tables, meter)?;
    source.with_inheritance_graph(dependencies, meter, |graph, meter| {
        let mut replay = Replay {
            graph,
            callables: &index,
            abstract_targets: abstract_targets::collect(source, dependencies, meter)?,
            adjustments: BTreeSet::new(),
        };
        inventory::validate(source, dispatch, &mut replay, meter)?;
        for binding in callables.entries() {
            meter.charge_work(1, &WirePath::root())?;
            if let mir::MirCallableOriginV1::Generated {
                callable,
                role:
                    GeneratedCallableKey::BoxingAdjust { .. }
                    | GeneratedCallableKey::DispatchAdjust { .. },
            } = binding.origin()
            {
                lookup(replay.adjustments.len(), meter)?;
                if !replay.adjustments.contains(callable) {
                    return Err(Error::UnexpectedAdjustment(*callable));
                }
            }
        }
        Ok(())
    })?
}

struct Replay<'g, 'c> {
    graph: &'g hir::CheckedNominalInheritanceGraphV1<'g>,
    callables: &'c dyn mir::MirTypeBridgeCallableLookupV1,
    abstract_targets:
        BTreeMap<(hir::SourceNominalId, PersistentDispatchSlotId), StrongCallableDefinitionOwner>,
    adjustments: BTreeSet<PersistentGeneratedCallableId>,
}

fn target(declaration: hir::InheritanceCallableDeclarationV1) -> StrongCallableDefinitionOwner {
    match declaration {
        hir::InheritanceCallableDeclarationV1::Function(id) => {
            StrongCallableDefinitionOwner::Function(id)
        }
        hir::InheritanceCallableDeclarationV1::Getter(id)
        | hir::InheritanceCallableDeclarationV1::Setter(id) => {
            StrongCallableDefinitionOwner::PropertyAccessor(id)
        }
    }
}

fn declaration(owner: hir::InheritanceCallableDeclarationV1) -> DispatchDeclarationOwner {
    match owner {
        hir::InheritanceCallableDeclarationV1::Function(id) => {
            DispatchDeclarationOwner::Function(id)
        }
        hir::InheritanceCallableDeclarationV1::Getter(id)
        | hir::InheritanceCallableDeclarationV1::Setter(id) => {
            DispatchDeclarationOwner::Accessor(id)
        }
    }
}

fn lookup(count: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(u64::from(count.max(1).ilog2()) + 1, &WirePath::root())?)
}

fn insert<T: Ord>(
    values: &mut BTreeSet<T>,
    value: T,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    lookup(values.len(), meter)?;
    if !values.contains(&value) {
        meter.check_table_entries(values.len() as u64 + 1, &WirePath::root())?;
        meter.charge_collection_slots(1, &WirePath::root())?;
        values.insert(value);
    }
    Ok(())
}
