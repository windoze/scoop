//! Ordinary-Cone lowering with compiler-protocol and dependency projections.

use super::*;

/// Lowers one ordinary HIR product against the exact MIR projections of all
/// selected external callables.
pub fn lower_ordinary<'core>(
    output: &scoop_hir::OrdinaryHirOutput,
    imported_core: mir::SelectedImportedMirSet<'core>,
    imported_dependencies: mir::SelectedDependencyMirSet,
) -> Result<mir::OrdinaryMirOutput<'core>, ImportedCoreMirLoweringError> {
    let hir = output.output().local.module();
    if !matches!(hir.core_protocols, hir::ConcreteCoreProtocols::Imported(_)) {
        return Err(ImportedCoreMirLoweringError::DefinedCoreProtocols);
    }
    if imported_dependencies.consumer() != hir.cone {
        return Err(
            ImportedCoreMirLoweringError::ForeignDependencyMirSelection {
                expected: hir.cone,
                actual: imported_dependencies.consumer(),
            },
        );
    }

    let (core_callables, cycle_authority) = lower_core_callables(output, &imported_core)?;
    let (dependency_callables, dependency_mapping) =
        lower_dependency_callables(output, &imported_dependencies)?;
    let module = lower_with_core_authority(
        &output.output().local,
        CoreMirLoweringAuthority::Imported,
        cycle_authority,
        core_callables,
        dependency_callables,
        dependency_mapping,
    );
    mir::OrdinaryMirOutput::try_new(module, imported_core, imported_dependencies)
        .map_err(ImportedCoreMirLoweringError::InvalidOutput)
}

type CoreCallableLowering = (
    Arena<mir::ImportedCoreCallableUse>,
    InitializationCycleLoweringAuthority,
);

fn lower_core_callables<'core>(
    output: &scoop_hir::OrdinaryHirOutput,
    imported: &mir::SelectedImportedMirSet<'core>,
) -> Result<CoreCallableLowering, ImportedCoreMirLoweringError> {
    let hir = output.output().local.module();
    let mut callables = Arena::new();
    let cycle_authority = lower_initialization_cycle_authority(hir, imported, &mut callables)?;
    if callables.len() != imported.len() {
        return Err(ImportedCoreMirLoweringError::UnusedMirCallable {
            selected: imported.len(),
            used: callables.len(),
        });
    }
    Ok((callables, cycle_authority))
}

fn lower_initialization_cycle_authority(
    hir: &hir::Module,
    imported: &mir::SelectedImportedMirSet<'_>,
    callables: &mut Arena<mir::ImportedCoreCallableUse>,
) -> Result<InitializationCycleLoweringAuthority, ImportedCoreMirLoweringError> {
    if hir.initialization_units.is_empty() {
        return Ok(InitializationCycleLoweringAuthority::ImportedUnused);
    }
    let hir::ConcreteCoreProtocols::Imported(protocols) = &hir.core_protocols else {
        unreachable!("ordinary HIR was checked to retain imported core protocols")
    };
    let scoop_hir::ImportedCoreProtocolCallableDefinition::Function(definition) = protocols
        .exceptions()
        .initialization_cycle_thrower()
        .definition()
    else {
        return Err(ImportedCoreMirLoweringError::InvalidInitializationCycleThrower);
    };
    let definition = definition.persistent();
    let id = imported
        .callable_for_kind(mir::CoreImportedCallableKind::InitializationCycleThrower)
        .ok_or(ImportedCoreMirLoweringError::MissingInitializationCycleThrower)?;
    let selected = imported
        .callable(id)
        .expect("a callable-kind lookup returns an in-bounds MIR callable");
    if selected.definition() != definition {
        return Err(ImportedCoreMirLoweringError::InitializationCycleThrowerMismatch);
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

type DependencyCallableLowering = (
    Arena<mir::ImportedDependencyMirCallableUse>,
    HashMap<hir::ImportedDependencyCallableUseId, mir::ImportedDependencyMirCallableId>,
);

fn lower_dependency_callables(
    output: &scoop_hir::OrdinaryHirOutput,
    imported: &mir::SelectedDependencyMirSet,
) -> Result<DependencyCallableLowering, ImportedCoreMirLoweringError> {
    let hir = output.output().local.module();
    let selected_hir = output.imported_dependencies();
    let mut callables = Arena::new();
    let mut mapping = HashMap::new();
    for (source_id, source) in hir.imported_dependency_callables.iter() {
        let selected = selected_hir.resolve_callable(source.reference()).ok_or(
            ImportedCoreMirLoweringError::ForeignDependencyHirCallable {
                index: source_id.into_raw().into_u32(),
            },
        )?;
        let capability = selected.capability();
        let id = imported
            .callable_for(selected.provider(), capability.declaration())
            .ok_or(ImportedCoreMirLoweringError::MissingDependencyMirCallable {
                index: source_id.into_raw().into_u32(),
            })?;
        let target = imported
            .callable(id)
            .expect("a dependency callable lookup returns an in-bounds MIR callable");
        if target.implementation() != capability.implementation() {
            return Err(
                ImportedCoreMirLoweringError::DependencyImplementationMismatch {
                    index: source_id.into_raw().into_u32(),
                },
            );
        }
        if target.signature() != capability.signature() {
            return Err(ImportedCoreMirLoweringError::DependencySignatureMismatch {
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
    if callables.len() != imported.len() {
        return Err(ImportedCoreMirLoweringError::UnusedDependencyMirCallable {
            selected: imported.len(),
            used: callables.len(),
        });
    }
    Ok((callables, mapping))
}

#[derive(Debug)]
pub enum ImportedCoreMirLoweringError {
    DefinedCoreProtocols,
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
    InvalidOutput(mir::OrdinaryMirOutputError),
}

impl std::fmt::Display for ImportedCoreMirLoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot lower imported ordinary HIR callables: {self:?}"
        )
    }
}

impl std::error::Error for ImportedCoreMirLoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidOutput(error) => Some(error),
            _ => None,
        }
    }
}
