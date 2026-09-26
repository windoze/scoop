//! Shared materialization of every external MIR callable root.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_lir as lir;
use scoop_mir as mir;

use crate::{LoweringContext, StrongLirLoweringError as Error, abi};

pub(super) fn lower_external_callables(
    context: &LoweringContext,
    input: &mir::SingleConeStrongMirInput,
    selected: &lir::SelectedExternalLirSet,
    layouts: Option<&lir::StrongProductionDependencySelectionV2<'_>>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> Result<
    (
        Arena<lir::ExternalCallable>,
        HashMap<mir::ExternalCallableUseId, lir::ExternalCallableId>,
    ),
    Error,
> {
    let roots = input.materialization().external_callable_roots();
    if selected.consumer() != input.module().cone {
        return Err(Error::ForeignExternalLirSelection {
            expected: input.module().cone,
            actual: selected.consumer(),
        });
    }
    if selected.len() > roots.len() || (layouts.is_none() && selected.len() != roots.len()) {
        return Err(Error::ExternalCallableCountMismatch {
            mir: roots.len(),
            lir: selected.len(),
        });
    }

    let module = input.module();
    let mut callables = Arena::new();
    let mut mapping = HashMap::with_capacity(roots.len());
    let mut direct_count = 0;
    for (index, root) in roots.iter().enumerate() {
        let provider = root.provider();
        let target = root.implementation();
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
                    .ok_or(Error::MissingExternalArgumentType {
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
            .ok_or(Error::MissingExternalResultType {
                index,
                exact: root.signature().result(),
            })?;
        let signature =
            abi::classify_mir_signature(context, parameters, result.ty(), structs, enums)?;
        let callable = if let Some(direct) = selected.callable_by_target(provider, target) {
            if direct.role() != root.role() {
                return Err(Error::ExternalCallableMismatch {
                    index,
                    provider,
                    target,
                });
            }
            direct_count += 1;
            direct
                .materialize(signature)
                .map_err(Error::ExternalCallable)?
        } else {
            layouts
                .ok_or(Error::MissingExternalCallable {
                    index,
                    provider,
                    target,
                })?
                .materialize_callable(provider, target, signature, enums)
                .map_err(Error::DependencyLayout)?
        };
        if callable.canonical_signature().signature() != root.signature() {
            return Err(Error::ExternalCallableMismatch {
                index,
                provider,
                target,
            });
        }
        let expected_gc = match root.gc_effect() {
            mir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
            mir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
        };
        if callable.canonical_signature().gc_effect() != expected_gc
            || callable.root_plan().canonical_gc_effect() != expected_gc
        {
            return Err(Error::ExternalCallableGcEffectMismatch {
                index,
                provider,
                target,
                mir: root.gc_effect(),
                lir: callable.canonical_signature().gc_effect(),
            });
        }
        let lir_id = callables.alloc(callable);
        mapping.insert(root.callable(), lir_id);
    }
    if direct_count != selected.len() {
        return Err(Error::ExternalCallableCountMismatch {
            mir: direct_count,
            lir: selected.len(),
        });
    }
    Ok((callables, mapping))
}
