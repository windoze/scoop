//! Dispatch replay from shared HIR schemas, declarations and actual selections.

use scoop_hir as hir;
use scoop_identity::{
    DispatchDeclarationOwner, GeneratedCallableKey, PersistentDispatchSlotId,
    PersistentExactTypeId, PersistentGeneratedCallableId, StrongCallableDefinitionOwner,
};
use scoop_mir as mir;
use scoop_wire::WirePath;
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
    direct_callables: &[&mir::CrossConeMirBridgeSectionV1],
    dispatch: &mir::CanonicalMirDispatchSchemasV1,
) -> Result<(), Error> {
    let mut tables = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut tables,
        dependency_callables.len() + 1,
        &WirePath::root(),
    )?;
    tables.push(callables);
    tables.extend_from_slice(dependency_callables);
    let index = mir::MirTypeBridgeCallableIndexV1::try_new(&tables, direct_callables)?;
    source.with_inheritance_graph(dependencies, |graph| {
        let mut replay = Replay {
            graph,
            callables: &index,
            abstract_targets: abstract_targets::collect(source, dependencies)?,
            adjustments: BTreeSet::new(),
        };
        inventory::validate(source, dispatch, &mut replay)?;
        for binding in callables.entries() {
            if let mir::MirCallableOriginV1::Generated {
                callable,
                role:
                    GeneratedCallableKey::BoxingAdjust { .. }
                    | GeneratedCallableKey::DispatchAdjust { .. },
            } = binding.origin()
            {
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

fn insert<T: Ord>(values: &mut BTreeSet<T>, value: T) -> Result<(), Error> {
    if !values.contains(&value) {
        values.insert(value);
    }
    Ok(())
}
