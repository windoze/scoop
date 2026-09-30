use std::collections::HashSet;

use super::{ConeMirInputError as Error, StrongExternalCallableRoot};
use crate::{Module, SelectedExternalMirSet};

/// Resolves every MIR use through one complete selection before projecting the
/// existing LIR metadata roles. The completed MIR output retains these roots.
pub(crate) fn validate_external_callables(
    module: &Module,
    selected: &SelectedExternalMirSet,
) -> Result<Vec<StrongExternalCallableRoot>, Error> {
    if selected.consumer() != module.cone {
        return Err(Error::ForeignExternalCallableSelection {
            expected: module.cone,
            actual: selected.consumer(),
        });
    }
    if module.meta.external_callables.len() != selected.len() {
        return Err(Error::ExternalCallableCountMismatch {
            module: module.meta.external_callables.len(),
            selected: selected.len(),
        });
    }
    let referenced = crate::external_callable::referenced_external_callables(module);
    let mut implementations = HashSet::new();
    let mut roots = Vec::with_capacity(selected.len());
    for (callable, value) in module.meta.external_callables.iter() {
        let index = callable.into_raw().into_u32();
        let selected = selected
            .resolve_callable(value.reference())
            .ok_or(Error::ForeignExternalCallable { index })?;
        if !implementations.insert(selected.implementation()) {
            return Err(Error::DuplicateExternalImplementation {
                implementation: selected.implementation(),
            });
        }
        if !referenced.contains(&callable) {
            return Err(Error::UnreferencedExternalCallable { index });
        }
        roots.push(StrongExternalCallableRoot {
            callable,
            provider: selected.provider(),
            implementation: selected.implementation(),
            signature: selected.signature().clone(),
            gc_effect: value.gc_effect(),
        });
    }
    for (id, bridge) in module.callback_bridges.iter() {
        let crate::StaticCallbackTarget::External {
            source,
            bridge_function,
        } = bridge.target
        else {
            continue;
        };
        let source = selected
            .resolve_callable(module.meta.external_callables[source].reference())
            .expect("external references were resolved above");
        let storage = selected
            .resolve_callable(module.meta.external_callables[bridge_function].reference())
            .expect("external references were resolved above");
        let scoop_identity::GeneratedCallableKey::StaticNoGcCallbackStorageBridge {
            signature, ..
        } = bridge.identity().callable_record().key()
        else {
            unreachable!("static bridge key")
        };
        if source.signature() != signature
            || storage.signature() != bridge.identity().signature_record().signature()
        {
            return Err(Error::ExternalCallbackSignatureMismatch { bridge: id });
        }
    }
    Ok(roots)
}
