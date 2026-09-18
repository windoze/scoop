//! Ordinary-Cone lowering with branded trusted-core and dependency inputs.

use super::*;

/// Lowers one ordinary HIR product against the exact MIR projections of all
/// selected external callables.
pub fn lower_ordinary<'core>(
    output: &scoop_hir::OrdinaryHirOutput<'core>,
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

    let (core_callables, core_mapping, cycle_authority) =
        lower_core_callables(output, &imported_core)?;
    let (dependency_callables, dependency_mapping) =
        lower_dependency_callables(output, &imported_dependencies)?;
    let module = lower_with_core_authority(
        &output.output().local,
        CoreMirLoweringAuthority::Imported,
        cycle_authority,
        core_callables,
        core_mapping,
        dependency_callables,
        dependency_mapping,
    );
    mir::OrdinaryMirOutput::try_new(module, imported_core, imported_dependencies)
        .map_err(ImportedCoreMirLoweringError::InvalidOutput)
}

type CoreCallableLowering = (
    Arena<mir::ImportedCoreCallableUse>,
    HashMap<hir::ImportedCoreCallableUseId, mir::ImportedCoreCallableUseId>,
    InitializationCycleLoweringAuthority,
);

fn lower_core_callables<'core>(
    output: &scoop_hir::OrdinaryHirOutput<'core>,
    imported: &mir::SelectedImportedMirSet<'core>,
) -> Result<CoreCallableLowering, ImportedCoreMirLoweringError> {
    let hir = output.output().local.module();
    let selected = output.imported_core();
    let mut callables = Arena::new();
    let mut mapping = HashMap::new();
    for (source_id, source) in hir.imported_core_callables.iter() {
        let selected_target = selected.resolve_callable(source.reference()).ok_or(
            ImportedCoreMirLoweringError::ForeignHirCallable {
                index: source_id.into_raw().into_u32(),
            },
        )?;
        let scoop_hir::ImportedCorePreludeTarget::Callable(target) = selected_target.target()
        else {
            return Err(ImportedCoreMirLoweringError::HirTargetIsNotCallable {
                index: source_id.into_raw().into_u32(),
            });
        };
        let scoop_hir::CoreHirCallableCapabilityV1::ParamFreeCandidate(signature) =
            target.capability()
        else {
            return Err(ImportedCoreMirLoweringError::HirCapabilityMismatch {
                index: source_id.into_raw().into_u32(),
            });
        };
        if signature.effect() != scoop_hir::concrete::Effect::Ordinary {
            return Err(ImportedCoreMirLoweringError::SuspendCallableUnavailable {
                index: source_id.into_raw().into_u32(),
            });
        }
        if signature.receiver().is_present() {
            return Err(ImportedCoreMirLoweringError::ReceiverCallableUnavailable {
                index: source_id.into_raw().into_u32(),
            });
        }
        let id = imported
            .callable_for_kind(mir::CoreImportedCallableKind::Prelude(
                selected_target.binding().persistent(),
            ))
            .ok_or(ImportedCoreMirLoweringError::MissingMirCallable {
                index: source_id.into_raw().into_u32(),
            })?;
        let mir_target = imported
            .callable(id)
            .expect("a binding lookup returns an in-bounds MIR callable");
        if mir_target.signature() != signature {
            return Err(ImportedCoreMirLoweringError::SignatureMismatch {
                index: source_id.into_raw().into_u32(),
            });
        }
        let target = callables.alloc(
            imported
                .callable_use(id)
                .expect("a selected MIR callable mints one branded use"),
        );
        mapping.insert(source_id, target);
    }

    let cycle_authority = lower_initialization_cycle_authority(hir, imported, &mut callables)?;
    if callables.len() != imported.len() {
        return Err(ImportedCoreMirLoweringError::UnusedMirCallable {
            selected: imported.len(),
            used: callables.len(),
        });
    }
    Ok((callables, mapping, cycle_authority))
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
    output: &scoop_hir::OrdinaryHirOutput<'_>,
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
    ForeignHirCallable {
        index: u32,
    },
    HirTargetIsNotCallable {
        index: u32,
    },
    HirCapabilityMismatch {
        index: u32,
    },
    SuspendCallableUnavailable {
        index: u32,
    },
    ReceiverCallableUnavailable {
        index: u32,
    },
    MissingMirCallable {
        index: u32,
    },
    SignatureMismatch {
        index: u32,
    },
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
