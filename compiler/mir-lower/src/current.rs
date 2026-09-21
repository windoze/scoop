//! Shared current-Cone lowering with protocol and dependency projections.

use super::*;

/// Lowers one complete HIR product while preserving its protocol selection type.
pub fn lower_current_cone<P: mir::MirProtocolSelection>(
    output: &scoop_hir::DependencyHirOutput,
    protocols: P,
    imported_dependencies: mir::SelectedDependencyMirSet,
) -> Result<mir::DependencyMirOutput<P>, CurrentConeMirLoweringError> {
    let hir = output.output().local.module();
    if imported_dependencies.consumer() != hir.cone {
        return Err(CurrentConeMirLoweringError::ForeignDependencyMirSelection {
            expected: hir.cone,
            actual: imported_dependencies.consumer(),
        });
    }
    let (authority, mut callables, cycle_authority) =
        match (&hir.core_protocols, protocols.as_strong_input()) {
            (
                hir::ConcreteCoreProtocols::Defined(defined),
                mir::StrongImportedCoreInput::Unused,
            ) => (
                CoreMirLoweringAuthority::Defined(defined.clone()),
                Arena::new(),
                InitializationCycleLoweringAuthority::Local,
            ),
            (
                hir::ConcreteCoreProtocols::Imported(_),
                mir::StrongImportedCoreInput::Selected(imported),
            ) => {
                let (callables, cycle) = lower_core_callables(output, imported)?;
                (CoreMirLoweringAuthority::Imported, callables, cycle)
            }
            _ => return Err(CurrentConeMirLoweringError::ProtocolOriginMismatch),
        };
    let dependency_mapping =
        lower_dependency_callables(output, &imported_dependencies, &mut callables)?;
    let module = lower_with_core_authority(
        &output.output().local,
        authority,
        cycle_authority,
        callables,
        dependency_mapping,
    );
    mir::DependencyMirOutput::try_new(module, protocols, imported_dependencies)
        .map_err(CurrentConeMirLoweringError::InvalidOutput)
}

type CoreCallableLowering = (
    Arena<mir::ExternalCallableUse>,
    InitializationCycleLoweringAuthority,
);

fn lower_core_callables<'core>(
    output: &scoop_hir::DependencyHirOutput,
    imported: &mir::SelectedImportedMirSet<'core>,
) -> Result<CoreCallableLowering, CurrentConeMirLoweringError> {
    let hir = output.output().local.module();
    let mut callables = Arena::new();
    let cycle_authority = lower_initialization_cycle_authority(hir, imported, &mut callables)?;
    if callables.len() != imported.len() {
        return Err(CurrentConeMirLoweringError::UnusedMirCallable {
            selected: imported.len(),
            used: callables.len(),
        });
    }
    Ok((callables, cycle_authority))
}

fn lower_initialization_cycle_authority(
    hir: &hir::Module,
    imported: &mir::SelectedImportedMirSet<'_>,
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
        .callable_for_kind(mir::CoreImportedCallableKind::InitializationCycleThrower)
        .ok_or(CurrentConeMirLoweringError::MissingInitializationCycleThrower)?;
    let selected = imported
        .callable(id)
        .expect("a callable-kind lookup returns an in-bounds MIR callable");
    if selected.definition() != definition {
        return Err(CurrentConeMirLoweringError::InitializationCycleThrowerMismatch);
    }
    let callable = callables.alloc(
        imported
            .callable_use(id)
            .expect("a selected MIR callable mints one branded use"),
    );
    Ok(InitializationCycleLoweringAuthority::Imported {
        definition,
        callable,
    })
}

fn lower_dependency_callables(
    output: &scoop_hir::DependencyHirOutput,
    imported: &mir::SelectedDependencyMirSet,
    callables: &mut Arena<mir::ExternalCallableUse>,
) -> Result<
    HashMap<hir::ImportedDependencyCallableUseId, mir::ExternalCallableUseId>,
    CurrentConeMirLoweringError,
> {
    let hir = output.output().local.module();
    let selected_hir = output.imported_dependencies();
    let mut mapping = HashMap::new();
    for (source_id, source) in hir.imported_dependency_callables.iter() {
        let selected = selected_hir.resolve_callable(source.reference()).ok_or(
            CurrentConeMirLoweringError::ForeignDependencyHirCallable {
                index: source_id.into_raw().into_u32(),
            },
        )?;
        let capability = selected.capability();
        let id = imported
            .callable_for(selected.provider(), capability.declaration())
            .ok_or(CurrentConeMirLoweringError::MissingDependencyMirCallable {
                index: source_id.into_raw().into_u32(),
            })?;
        let target = imported
            .callable(id)
            .expect("a dependency callable lookup returns an in-bounds MIR callable");
        if target.implementation() != capability.implementation() {
            return Err(
                CurrentConeMirLoweringError::DependencyImplementationMismatch {
                    index: source_id.into_raw().into_u32(),
                },
            );
        }
        if target.signature() != capability.signature() {
            return Err(CurrentConeMirLoweringError::DependencySignatureMismatch {
                index: source_id.into_raw().into_u32(),
            });
        }
        let effect = match capability.is_no_gc() {
            false => mir::GcEffect::Managed,
            true => mir::GcEffect::NoGc,
        };
        let target = callables.alloc(
            imported
                .callable_use(id, effect)
                .expect("a selected dependency MIR callable mints one branded use"),
        );
        mapping.insert(source_id, target);
    }
    if mapping.len() != imported.len() {
        return Err(CurrentConeMirLoweringError::UnusedDependencyMirCallable {
            selected: imported.len(),
            used: mapping.len(),
        });
    }
    Ok(mapping)
}

#[derive(Debug)]
pub enum CurrentConeMirLoweringError {
    ProtocolOriginMismatch,
    InvalidInitializationCycleThrower,
    MissingInitializationCycleThrower,
    InitializationCycleThrowerMismatch,
    UnusedMirCallable {
        selected: usize,
        used: usize,
    },
    ForeignDependencyMirSelection {
        expected: mir::ConeIdentity,
        actual: mir::ConeIdentity,
    },
    ForeignDependencyHirCallable {
        index: u32,
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
    UnusedDependencyMirCallable {
        selected: usize,
        used: usize,
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
            Self::InvalidOutput(error) => Some(error),
            _ => None,
        }
    }
}
