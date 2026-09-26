//! Shared current-Cone lowering with protocol and dependency projections.

use super::*;

/// Lowers one complete HIR product with its unified external callable selection.
pub fn lower_current_cone(
    output: &scoop_hir::DependencyHirOutput,
    selected_callables: mir::SelectedExternalMirSet,
) -> Result<mir::DependencyMirOutput, CurrentConeMirLoweringError> {
    let hir = output.output().local.module();
    if selected_callables.consumer() != hir.cone {
        return Err(CurrentConeMirLoweringError::ForeignExternalMirSelection {
            expected: hir.cone,
            actual: selected_callables.consumer(),
        });
    }
    let mut callables = Arena::new();
    let cycle_authority = match &hir.core_protocols {
        hir::ConcreteCoreProtocols::Defined(_) => InitializationCycleLoweringAuthority::Local,
        hir::ConcreteCoreProtocols::Imported(_) => {
            lower_initialization_cycle_authority(hir, &selected_callables, &mut callables)?
        }
    };
    let dependency_mapping =
        lower_dependency_callables(output, &selected_callables, &mut callables)?;
    lower_runtime_constructors(hir, &selected_callables, &mut callables)?;
    let module = lower_with_dependencies(
        &output.output().local,
        cycle_authority,
        callables,
        dependency_mapping,
    );
    mir::DependencyMirOutput::try_new(module, selected_callables)
        .map_err(CurrentConeMirLoweringError::InvalidOutput)
}

fn lower_initialization_cycle_authority(
    hir: &hir::Module,
    imported: &mir::SelectedExternalMirSet,
    callables: &mut Arena<mir::ExternalCallableUse>,
) -> Result<InitializationCycleLoweringAuthority, CurrentConeMirLoweringError> {
    if hir.initialization_units.is_empty() {
        return Ok(InitializationCycleLoweringAuthority::ImportedUnused);
    }
    let hir::ConcreteCoreProtocols::Imported(protocols) = &hir.core_protocols else {
        unreachable!("HIR and MIR inputs were checked to retain imported protocols")
    };
    let scoop_hir::ImportedCoreProtocolCallableDefinition::Function(definition) = protocols
        .exceptions()
        .initialization_cycle_thrower()
        .definition()
    else {
        return Err(CurrentConeMirLoweringError::InvalidInitializationCycleThrower);
    };
    let definition = definition.persistent();
    let id = imported
        .initialization_cycle()
        .ok_or(CurrentConeMirLoweringError::MissingInitializationCycleThrower)?;
    let selected = imported
        .resolve_callable(id)
        .expect("a callable-kind lookup returns an in-bounds MIR callable");
    if selected.implementation()
        != scoop_identity::StrongCallableDefinitionOwner::Function(definition)
    {
        return Err(CurrentConeMirLoweringError::InitializationCycleThrowerMismatch);
    }
    let callable = callables.alloc(
        imported
            .callable_use(id, mir::GcEffect::Managed)
            .expect("a selected MIR callable has a complete typed reference"),
    );
    Ok(InitializationCycleLoweringAuthority::Imported {
        definition,
        callable,
    })
}

fn lower_dependency_callables(
    output: &scoop_hir::DependencyHirOutput,
    imported: &mir::SelectedExternalMirSet,
    callables: &mut Arena<mir::ExternalCallableUse>,
) -> Result<
    HashMap<
        hir::ImportedDependencyCallableUseId,
        (mir::ExternalCallableUseId, mir::MirCallableLoweringRoleV1),
    >,
    CurrentConeMirLoweringError,
> {
    let mut mapping = HashMap::new();

    let executable = output
        .executable_dependency_callables()
        .map_err(CurrentConeMirLoweringError::Occurrences)?;
    for use_ in executable {
        let source_id = use_.callee();
        let selected = use_.callable();
        let capability = selected.capability();
        let id = imported
            .callable_for(selected.provider(), capability.implementation())
            .ok_or(CurrentConeMirLoweringError::MissingDependencyMirCallable {
                index: source_id.into_raw().into_u32(),
            })?;
        let target = imported
            .resolve_callable(id)
            .expect("a dependency callable lookup returns an in-bounds MIR callable");
        if target.implementation() != capability.implementation() {
            return Err(
                CurrentConeMirLoweringError::DependencyImplementationMismatch {
                    index: source_id.into_raw().into_u32(),
                },
            );
        }
        if target.semantic_signature() != capability.signature() {
            return Err(CurrentConeMirLoweringError::DependencySignatureMismatch {
                index: source_id.into_raw().into_u32(),
            });
        }
        let effect = match capability.is_no_gc() {
            false => mir::GcEffect::Managed,
            true => mir::GcEffect::NoGc,
        };
        let lowering_role = target.lowering_role();
        let target = callables.alloc(
            imported
                .callable_use(id, effect)
                .expect("a selected dependency MIR callable has a complete typed reference"),
        );
        mapping.insert(source_id, (target, lowering_role));
    }
    Ok(mapping)
}

pub(super) fn runtime_constructor_target(
    constructor: &scoop_hir::ImportedCoreProtocolCallable,
) -> Result<
    (
        mir::ConeIdentity,
        scoop_identity::StrongCallableDefinitionOwner,
    ),
    CurrentConeMirLoweringError,
> {
    use scoop_hir::ImportedCoreProtocolCallableDefinition as Definition;
    use scoop_identity::StrongCallableDefinitionOwner as Target;
    let target = match constructor.definition() {
        Definition::Constructor(id) => Target::Constructor(id.persistent()),
        Definition::GeneratedCallable(id) => Target::GeneratedCallable(id.persistent()),
        _ => return Err(CurrentConeMirLoweringError::InvalidRuntimeConstructor),
    };
    Ok((constructor.provider(), target))
}

fn lower_runtime_constructors(
    module: &hir::Module,
    selected: &mir::SelectedExternalMirSet,
    callables: &mut Arena<mir::ExternalCallableUse>,
) -> Result<(), CurrentConeMirLoweringError> {
    let hir::ConcreteCoreProtocols::Imported(protocols) = &module.core_protocols else {
        return Ok(());
    };
    for constructor in [
        protocols.exceptions().class_cast_exception_constructor(),
        protocols.exceptions().arithmetic_exception_constructor(),
    ] {
        let (provider, target) = runtime_constructor_target(constructor)?;
        if callables.iter().any(|(_, callable)| {
            callable.reference().provider() == provider
                && callable.reference().implementation() == target
        }) {
            continue;
        }
        if let Some(id) = selected.callable_for(provider, target) {
            callables.alloc(
                selected
                    .callable_use(id, mir::GcEffect::Managed)
                    .expect("the selected constructor retains its complete physical signature"),
            );
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum CurrentConeMirLoweringError {
    Occurrences(scoop_hir::DependencyCallOccurrenceError),
    InvalidInitializationCycleThrower,
    MissingInitializationCycleThrower,
    InitializationCycleThrowerMismatch,
    InvalidRuntimeConstructor,
    ForeignExternalMirSelection {
        expected: mir::ConeIdentity,
        actual: mir::ConeIdentity,
    },
    MissingDependencyMirCallable {
        index: u32,
    },
    DependencyImplementationMismatch {
        index: u32,
    },
    DependencySignatureMismatch {
        index: u32,
    },
    InvalidOutput(mir::DependencyMirOutputError),
}

impl std::fmt::Display for CurrentConeMirLoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot lower current-Cone HIR callables: {self:?}"
        )
    }
}

impl std::error::Error for CurrentConeMirLoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Occurrences(error) => Some(error),
            Self::InvalidOutput(error) => Some(error),
            _ => None,
        }
    }
}
