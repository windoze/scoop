//! Shared materialization of every external MIR callable root.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_lir as lir;
use scoop_mir as mir;

use crate::{LoweringContext, StrongLirLoweringError, abi};

pub(super) fn lower_external_callables(
    context: &LoweringContext,
    input: &mir::SingleConeStrongMirInput,
    selected: &lir::SelectedExternalLirSet,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> Result<
    (
        Arena<lir::ExternalCallable>,
        HashMap<mir::ExternalCallableUseId, lir::ExternalCallableId>,
    ),
    StrongLirLoweringError,
> {
    let roots = input.materialization().external_callable_roots();
    if selected.consumer() != input.module().cone {
        return Err(StrongLirLoweringError::ForeignExternalLirSelection {
            expected: input.module().cone,
            actual: selected.consumer(),
        });
    }
    if selected.len() != roots.len() {
        return Err(StrongLirLoweringError::ExternalCallableCountMismatch {
            mir: roots.len(),
            lir: selected.len(),
        });
    }

    let module = input.module();
    let mut callables = Arena::new();
    let mut mapping = HashMap::with_capacity(roots.len());
    for (index, root) in roots.iter().enumerate() {
        let id = selected
            .callable_for(root.provider(), root.declaration())
            .ok_or(StrongLirLoweringError::MissingExternalCallable {
                index,
                provider: root.provider(),
                declaration: root.declaration(),
            })?;
        let authority = selected
            .callable(id)
            .expect("a selected dependency key maps to an in-bounds LIR authority");
        let bridge = authority.bridge();
        if authority.role() != root.role()
            || bridge.target() != root.implementation()
            || bridge.abi_signature().signature() != root.signature()
        {
            return Err(StrongLirLoweringError::ExternalCallableMismatch {
                index,
                provider: root.provider(),
                declaration: root.declaration(),
            });
        }
        let expected_gc = match root.gc_effect() {
            mir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
            mir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
        };
        if bridge.abi_signature().gc_effect() != expected_gc
            || bridge.root_plan().canonical_gc_effect() != expected_gc
        {
            return Err(StrongLirLoweringError::ExternalCallableGcEffectMismatch {
                index,
                provider: root.provider(),
                declaration: root.declaration(),
                mir: root.gc_effect(),
                lir: bridge.abi_signature().gc_effect(),
            });
        }

        let exact_arguments = root
            .signature()
            .receiver()
            .into_option()
            .into_iter()
            .chain(root.signature().parameters().iter().copied());
        let parameters = exact_arguments
            .enumerate()
            .map(|(argument, exact)| {
                module
                    .meta
                    .source_exact_types
                    .get_by_identity(exact)
                    .map(|identity| identity.ty())
                    .ok_or(StrongLirLoweringError::MissingExternalArgumentType {
                        index,
                        argument,
                        exact,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = module
            .meta
            .source_exact_types
            .get_by_identity(root.signature().result())
            .ok_or(StrongLirLoweringError::MissingExternalResultType {
                index,
                exact: root.signature().result(),
            })?;
        let signature =
            abi::classify_mir_signature(context, parameters, result.ty(), structs, enums)?;
        let callable = authority
            .materialize(signature)
            .map_err(StrongLirLoweringError::ExternalCallable)?;
        let lir_id = callables.alloc(callable);
        mapping.insert(root.callable(), lir_id);
    }
    Ok((callables, mapping))
}
