//! Dispatch replay from shared HIR schemas, declarations and actual selections.

use scoop_hir as hir;
use scoop_identity::{
    CallableDefinitionOwner, DispatchDeclarationOwner, GeneratedCallableKey,
    PersistentDispatchSlotId, PersistentExactTypeId, PersistentGeneratedCallableId,
    StrongCallableDefinitionOwner,
};
use scoop_mir as mir;
use scoop_wire::WirePath;
use std::collections::BTreeSet;

mod bindings;
mod entries;
mod errors;
mod inventory;
mod signatures;
use SharedMirDispatchComponent as Component;
use SharedMirDispatchValidationError as Error;
pub use errors::{SharedMirDispatchComponent, SharedMirDispatchValidationError};

pub fn validate_shared_mir_dispatch(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    core: Option<&hir::CoreCoroutineProtocolV1>,
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
    let mut replay = Replay {
        callables: &index,
        identities: source.metadata().identities,
        core,
        adjustments: BTreeSet::new(),
    };
    inventory::validate(source, dispatch, &mut replay)?;
    for binding in callables.entries() {
        if let mir::MirCallableOriginV1::Generated {
            callable,
            role:
                GeneratedCallableKey::BoxingAdjust { .. } | GeneratedCallableKey::DispatchAdjust { .. },
        } = binding.origin()
        {
            if !replay.adjustments.contains(callable) {
                return Err(Error::UnexpectedAdjustment(*callable));
            }
        }
    }
    Ok(())
}

struct Replay<'c> {
    callables: &'c dyn mir::MirTypeBridgeCallableLookupV1,
    identities: &'c scoop_identity::ValidatedIdentityGraph,
    core: Option<&'c hir::CoreCoroutineProtocolV1>,
    adjustments: BTreeSet<PersistentGeneratedCallableId>,
}

impl Replay<'_> {
    fn target(
        &self,
        source: &hir::InheritanceSlotTargetV1,
    ) -> Result<CallableDefinitionOwner, Error> {
        mir::dispatch_exact_declaration_target(
            self.identities,
            declaration(source.declaration()),
            source.signature().receiver(),
        )
        .map_err(Error::Callable)
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
