//! Materialization of ordinary-dependency MIR roots into typed LIR externals.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_lir as lir;
use scoop_mir as mir;

use crate::{LoweringContext, StrongLirLoweringError, abi};

/// Ordinary-dependency LIR authority paired with one sealed MIR input.
///
/// The selected branch must cover the MIR dependency roots exactly. `Unused`
/// is legal only when that root set is empty.
#[derive(Clone, Copy)]
pub enum StrongImportedDependencyLirInput<'a> {
    Unused,
    Selected(&'a lir::SelectedDependencyLirSet),
}

pub(super) fn lower_imported_dependency_callables(
    context: &LoweringContext,
    input: &mir::SingleConeStrongMirInput,
    imported: StrongImportedDependencyLirInput<'_>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    callables: &mut Arena<lir::ExternalCallable>,
    mapping: &mut HashMap<mir::ExternalCallableUseId, lir::ExternalCallableId>,
) -> Result<(), StrongLirLoweringError> {
    let roots = input.materialization().imported_dependency_callable_roots();
    let selected = match imported {
        StrongImportedDependencyLirInput::Unused if roots.is_empty() => {
            return Ok(());
        }
        StrongImportedDependencyLirInput::Unused => {
            return Err(StrongLirLoweringError::MissingImportedDependencyLirAuthority);
        }
        StrongImportedDependencyLirInput::Selected(selected) => selected,
    };
    if selected.consumer() != input.module().cone {
        return Err(
            StrongLirLoweringError::ForeignImportedDependencyLirSelection {
                expected: input.module().cone,
                actual: selected.consumer(),
            },
        );
    }
    if selected.len() != roots.len() {
        return Err(StrongLirLoweringError::ImportedDependencyLirCountMismatch {
            mir: roots.len(),
            lir: selected.len(),
        });
    }

    let module = input.module();
    for (index, root) in roots.iter().enumerate() {
        let id = selected
            .callable_for(root.provider(), root.declaration())
            .ok_or(
                StrongLirLoweringError::MissingImportedDependencyLirCallable {
                    index,
                    provider: root.provider(),
                    declaration: root.declaration(),
                },
            )?;
        let authority = selected
            .callable(id)
            .expect("a selected dependency key maps to an in-bounds LIR authority");
        let bridge = authority.bridge();
        if bridge.target() != root.implementation()
            || bridge.abi_signature().signature() != root.signature()
        {
            return Err(
                StrongLirLoweringError::ImportedDependencyLirCallableMismatch {
                    index,
                    provider: root.provider(),
                    declaration: root.declaration(),
                },
            );
        }
        let expected_gc = match root.gc_effect() {
            mir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
            mir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
        };
        if bridge.abi_signature().gc_effect() != expected_gc
            || bridge.root_plan().gc_effect() != expected_gc
        {
            return Err(
                StrongLirLoweringError::ImportedDependencyLirGcEffectMismatch {
                    index,
                    provider: root.provider(),
                    declaration: root.declaration(),
                    mir: root.gc_effect(),
                    lir: bridge.abi_signature().gc_effect(),
                },
            );
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
                    .ok_or(
                        StrongLirLoweringError::MissingImportedDependencyArgumentType {
                            index,
                            argument,
                            exact,
                        },
                    )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = module
            .meta
            .source_exact_types
            .get_by_identity(root.signature().result())
            .ok_or(
                StrongLirLoweringError::MissingImportedDependencyResultType {
                    index,
                    exact: root.signature().result(),
                },
            )?;
        let signature =
            abi::classify_mir_signature(context, parameters, result.ty(), structs, enums)?;
        let callable = authority
            .materialize(signature)
            .map_err(StrongLirLoweringError::ImportedDependencyCallable)?;
        let lir_id = callables.alloc(callable);
        mapping.insert(root.callable(), lir_id);
    }
    Ok(())
}
