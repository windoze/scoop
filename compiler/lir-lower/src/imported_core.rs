use super::*;

/// Imported-core LIR authority paired with one sealed MIR input.
///
/// The selected branch must cover the MIR materialization roots exactly.
/// `Unused` is legal only when that root set is empty.
#[derive(Clone, Copy)]
pub enum StrongImportedCoreLirInput<'a> {
    Unused,
    Selected(&'a lir::SelectedImportedLirSet<'a>),
    #[cfg(test)]
    TestRuntimeString(scoop_identity::PersistentExactTypeId),
}

pub(super) fn lower_imported_core_runtime_string(
    input: &mir::SingleConeStrongMirInput,
    imported: StrongImportedCoreLirInput<'_>,
) -> Result<
    (
        Arena<lir::CoreExternalTypeDescriptor>,
        Option<lir::TypeDescriptorRef>,
    ),
    StrongLirLoweringError,
> {
    match (input.module().cone == lir::ConeIdentity::CORE, imported) {
        (true, StrongImportedCoreLirInput::Unused) => Ok((Arena::new(), None)),
        (true, StrongImportedCoreLirInput::Selected(_)) => {
            Err(StrongLirLoweringError::CoreCannotImportCore)
        }
        (false, StrongImportedCoreLirInput::Unused) => {
            Err(StrongLirLoweringError::MissingImportedCoreLirAuthority)
        }
        (false, StrongImportedCoreLirInput::Selected(selected)) => {
            let actual = selected.runtime_string().target();
            if let Some(expected) = runtime_string_exact_type(input) {
                if actual != expected {
                    return Err(StrongLirLoweringError::ImportedCoreRuntimeStringMismatch {
                        mir: expected,
                        lir: actual,
                    });
                }
            }
            let mut descriptors = Arena::new();
            let descriptor = selected
                .runtime_string()
                .materialize()
                .map_err(StrongLirLoweringError::ImportedCoreTypeDescriptor)?;
            let reference = lir::TypeDescriptorRef::CoreExternal(descriptors.alloc(descriptor));
            Ok((descriptors, Some(reference)))
        }
        #[cfg(test)]
        (true, StrongImportedCoreLirInput::TestRuntimeString(_)) => {
            Err(StrongLirLoweringError::CoreCannotImportCore)
        }
        #[cfg(test)]
        (false, StrongImportedCoreLirInput::TestRuntimeString(target)) => {
            if let Some(expected) = runtime_string_exact_type(input)
                && target != expected
            {
                return Err(StrongLirLoweringError::ImportedCoreRuntimeStringMismatch {
                    mir: expected,
                    lir: target,
                });
            }
            let mut descriptors = Arena::new();
            let descriptor = lir::CoreExternalTypeDescriptor::new(target)
                .map_err(StrongLirLoweringError::ImportedCoreTypeDescriptor)?;
            let reference = lir::TypeDescriptorRef::CoreExternal(descriptors.alloc(descriptor));
            Ok((descriptors, Some(reference)))
        }
    }
}

fn runtime_string_exact_type(
    input: &mir::SingleConeStrongMirInput,
) -> Option<scoop_identity::PersistentExactTypeId> {
    input
        .module()
        .meta
        .source_exact_types
        .get(&mir::Type::String)
        .map(|identity| identity.identity_record().id())
}

pub(super) fn lower_imported_core_callables(
    context: &LoweringContext,
    input: &mir::SingleConeStrongMirInput,
    imported: StrongImportedCoreLirInput<'_>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> Result<
    (
        Arena<lir::CoreExternalCallable>,
        HashMap<mir::ImportedCoreCallableUseId, lir::CoreExternalCallableId>,
    ),
    StrongLirLoweringError,
> {
    let roots = input.materialization().imported_core_callable_roots();
    let selected = match imported {
        StrongImportedCoreLirInput::Unused if roots.is_empty() => {
            return Ok((Arena::new(), HashMap::new()));
        }
        StrongImportedCoreLirInput::Unused => {
            return Err(StrongLirLoweringError::MissingImportedCoreLirAuthority);
        }
        StrongImportedCoreLirInput::Selected(_)
            if input.module().cone == lir::ConeIdentity::CORE =>
        {
            return Err(StrongLirLoweringError::CoreCannotImportCore);
        }
        StrongImportedCoreLirInput::Selected(selected) => selected,
        #[cfg(test)]
        StrongImportedCoreLirInput::TestRuntimeString(_) if roots.is_empty() => {
            return Ok((Arena::new(), HashMap::new()));
        }
        #[cfg(test)]
        StrongImportedCoreLirInput::TestRuntimeString(_) => {
            return Err(StrongLirLoweringError::MissingImportedCoreLirAuthority);
        }
    };
    if selected.len() != roots.len() {
        return Err(StrongLirLoweringError::ImportedCoreLirCountMismatch {
            mir: roots.len(),
            lir: selected.len(),
        });
    }

    let module = input.module();
    let mut callables = Arena::new();
    let mut mapping = HashMap::with_capacity(roots.len());
    for (index, root) in roots.iter().enumerate() {
        let id = selected.callable_for_binding(root.binding()).ok_or(
            StrongLirLoweringError::MissingImportedCoreLirCallable {
                index,
                binding: root.binding(),
            },
        )?;
        let authority = selected
            .callable(id)
            .expect("a selected binding maps to an in-bounds LIR authority");
        if authority.target() != root.implementation() || authority.signature() != root.signature()
        {
            return Err(StrongLirLoweringError::ImportedCoreLirCallableMismatch {
                index,
                binding: root.binding(),
            });
        }
        let parameters = root
            .signature()
            .parameters()
            .iter()
            .enumerate()
            .map(|(parameter, exact)| {
                module
                    .meta
                    .source_exact_types
                    .get_by_identity(*exact)
                    .map(|identity| identity.ty())
                    .ok_or(StrongLirLoweringError::MissingImportedCoreParameterType {
                        index,
                        parameter,
                        exact: *exact,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = module
            .meta
            .source_exact_types
            .get_by_identity(root.signature().result())
            .ok_or(StrongLirLoweringError::MissingImportedCoreResultType {
                index,
                exact: root.signature().result(),
            })?;
        let signature =
            abi::classify_mir_signature(context, parameters, result.ty(), structs, enums);
        let callable = authority
            .materialize(signature)
            .map_err(StrongLirLoweringError::ImportedCoreCallable)?;
        let lir_id = callables.alloc(callable);
        mapping.insert(root.callable(), lir_id);
    }
    Ok((callables, mapping))
}
