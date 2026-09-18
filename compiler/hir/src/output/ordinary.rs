use std::collections::HashSet;
use std::fmt;

use crate::{CoreProtocols, LocalConcreteMaterializationContract, concrete};

/// Closed ordinary HIR product whose imported-core uses are bound to the
/// exact selected set that admitted them.
///
/// This wrapper is the only ordinary product accepted by the imported-core
/// MIR path. Keeping the sidecars here prevents a caller from pairing the HIR
/// graph with selections projected from other trusted artifacts.
pub struct OrdinaryHirOutput<'a> {
    output: crate::Output,
    imported_core: crate::SelectedImportedCoreSet<'a>,
    imported_dependencies: crate::SelectedImportedDependencySet,
    concrete_dependency_witness_uses: Vec<crate::ExternalHirBindingWitnessUse>,
}

impl<'a> OrdinaryHirOutput<'a> {
    pub fn try_new(
        output: crate::Output,
        imported_core: crate::SelectedImportedCoreSet<'a>,
        imported_dependencies: crate::SelectedImportedDependencySet,
    ) -> Result<Self, OrdinaryHirOutputError> {
        if output.export.module().cone == scoop_identity::ConeIdentity::CORE {
            return Err(OrdinaryHirOutputError::CurrentConeIsCore);
        }
        if !matches!(
            (
                &output.export.module().core_protocols,
                &output.local.module().core_protocols
            ),
            (
                CoreProtocols::Imported(_),
                concrete::ConcreteCoreProtocols::Imported(_)
            )
        ) {
            return Err(OrdinaryHirOutputError::DefinedCoreProtocols);
        }
        if !matches!(
            output.local.materialization(),
            LocalConcreteMaterializationContract::Ordinary
        ) {
            return Err(OrdinaryHirOutputError::CoreShapeSupportMaterialization);
        }
        if imported_dependencies.consumer() != output.export.module().cone {
            return Err(OrdinaryHirOutputError::DependencyConsumerMismatch {
                output: output.export.module().cone,
                selected: imported_dependencies.consumer(),
            });
        }
        let export = output.export.module();
        let local = output.local.module();
        validate_imported_dependency_projection(export, local, &imported_dependencies)?;
        let selected_count = imported_core.callable_count()
            + imported_core.type_count()
            + imported_core.value_count();
        let mut bindings = HashSet::with_capacity(selected_count);
        validate_imported_core_projection(
            ImportedCoreUseKind::Callable,
            export
                .imported_core_callables
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            local
                .imported_core_callables
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            imported_core.callable_count(),
            |reference| imported_core.resolve_callable(reference),
            &mut bindings,
        )?;
        validate_imported_core_projection(
            ImportedCoreUseKind::Type,
            export
                .imported_core_types
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            local
                .imported_core_types
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            imported_core.type_count(),
            |reference| imported_core.resolve_type(reference),
            &mut bindings,
        )?;
        validate_imported_core_projection(
            ImportedCoreUseKind::Value,
            export
                .imported_core_values
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            local
                .imported_core_values
                .iter()
                .map(|(id, use_)| (id.into_raw().into_u32(), use_.reference())),
            imported_core.value_count(),
            |reference| imported_core.resolve_value(reference),
            &mut bindings,
        )?;

        let concrete_dependency_witness_uses =
            concrete_dependency_witness_uses(&imported_dependencies);

        Ok(Self {
            output,
            imported_core,
            imported_dependencies,
            concrete_dependency_witness_uses,
        })
    }

    pub const fn output(&self) -> &crate::Output {
        &self.output
    }

    pub const fn imported_core(&self) -> &crate::SelectedImportedCoreSet<'a> {
        &self.imported_core
    }

    pub const fn imported_dependencies(&self) -> &crate::SelectedImportedDependencySet {
        &self.imported_dependencies
    }

    /// Canonical source-name proofs for every committed ordinary-dependency
    /// HIR use. These are derived from the winner-only selection transaction,
    /// never reconstructed from transient local-concrete nodes.
    pub fn concrete_dependency_witness_uses(&self) -> &[crate::ExternalHirBindingWitnessUse] {
        &self.concrete_dependency_witness_uses
    }

    pub fn into_parts(
        self,
    ) -> (
        crate::Output,
        crate::SelectedImportedCoreSet<'a>,
        crate::SelectedImportedDependencySet,
    ) {
        (self.output, self.imported_core, self.imported_dependencies)
    }
}

fn concrete_dependency_witness_uses(
    selected: &crate::SelectedImportedDependencySet,
) -> Vec<crate::ExternalHirBindingWitnessUse> {
    let mut uses = Vec::new();
    for callable in selected.callables() {
        let target = crate::ExternalHirTargetV1::Callable(callable.interface().declaration());
        append_concrete_dependency_witnesses(&mut uses, target, callable.binding());
    }
    for constant in selected.constants() {
        let target = crate::ExternalHirTargetV1::Property(scoop_identity::PropertyOwner::Property(
            constant.record().property(),
        ));
        append_concrete_dependency_witnesses(&mut uses, target, constant.binding());
    }
    uses.sort_unstable();
    uses.dedup();
    uses
}

fn append_concrete_dependency_witnesses(
    uses: &mut Vec<crate::ExternalHirBindingWitnessUse>,
    target: crate::ExternalHirTargetV1,
    binding: &crate::DirectImportedTargetBinding,
) {
    uses.extend(binding.sources().map(|source| {
        crate::ExternalHirBindingWitnessUse::new(
            target,
            crate::ExternalHirBindingWitnessRole::ConcreteSelectedUse,
            source.witness().dependency().clone(),
        )
    }));
}

fn validate_imported_dependency_projection(
    export: &crate::ExportHir,
    local: &concrete::Module,
    selected: &crate::SelectedImportedDependencySet,
) -> Result<(), OrdinaryHirOutputError> {
    let export_count = export.imported_dependency_callables.len();
    let local_count = local.imported_dependency_callables.len();
    if export_count != local_count {
        return Err(OrdinaryHirOutputError::DependencyProjectionCountMismatch {
            export: export_count,
            local: local_count,
        });
    }
    if export_count != selected.callable_count() {
        return Err(OrdinaryHirOutputError::DependencySelectionCountMismatch {
            hir: export_count,
            selected: selected.callable_count(),
        });
    }

    let mut references = HashSet::with_capacity(export_count);
    for ((export_id, export_use), (local_id, local_use)) in export
        .imported_dependency_callables
        .iter()
        .zip(local.imported_dependency_callables.iter())
    {
        let index = export_id.into_raw().into_u32();
        if export_id.into_raw() != local_id.into_raw()
            || export_use.reference() != local_use.reference()
        {
            return Err(OrdinaryHirOutputError::DependencyProjectionMismatch { index });
        }
        let reference = export_use.reference();
        if selected.resolve_callable(reference).is_none() {
            return Err(OrdinaryHirOutputError::ForeignImportedDependencyUse { index });
        }
        if !references.insert(reference) {
            return Err(OrdinaryHirOutputError::DuplicateImportedDependencyUse { index });
        }
    }
    Ok(())
}

fn validate_imported_core_projection<'a, Reference: Copy + Eq>(
    kind: ImportedCoreUseKind,
    export: impl ExactSizeIterator<Item = (u32, Reference)>,
    local: impl ExactSizeIterator<Item = (u32, Reference)>,
    selected_count: usize,
    mut resolve: impl FnMut(Reference) -> Option<crate::SelectedImportedCoreTarget<'a>>,
    bindings: &mut HashSet<scoop_identity::PersistentExportBindingId>,
) -> Result<(), OrdinaryHirOutputError> {
    let export_count = export.len();
    let local_count = local.len();
    if export_count != local_count {
        return Err(OrdinaryHirOutputError::ProjectionCountMismatch {
            kind,
            export: export_count,
            local: local_count,
        });
    }
    if export_count != selected_count {
        return Err(OrdinaryHirOutputError::SelectionCountMismatch {
            kind,
            hir: export_count,
            selected: selected_count,
        });
    }
    for ((export_index, export_reference), (local_index, local_reference)) in export.zip(local) {
        if export_index != local_index || export_reference != local_reference {
            return Err(OrdinaryHirOutputError::ProjectionMismatch {
                kind,
                index: export_index,
            });
        }
        let Some(selected) = resolve(export_reference) else {
            return Err(OrdinaryHirOutputError::ForeignImportedCoreUse {
                kind,
                index: export_index,
            });
        };
        if !bindings.insert(selected.binding().persistent()) {
            return Err(OrdinaryHirOutputError::DuplicateImportedCoreBinding {
                kind,
                index: export_index,
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedCoreUseKind {
    Callable,
    Type,
    Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrdinaryHirOutputError {
    CurrentConeIsCore,
    DefinedCoreProtocols,
    CoreShapeSupportMaterialization,
    DependencyConsumerMismatch {
        output: scoop_identity::ConeIdentity,
        selected: scoop_identity::ConeIdentity,
    },
    DependencyProjectionCountMismatch {
        export: usize,
        local: usize,
    },
    DependencySelectionCountMismatch {
        hir: usize,
        selected: usize,
    },
    DependencyProjectionMismatch {
        index: u32,
    },
    ForeignImportedDependencyUse {
        index: u32,
    },
    DuplicateImportedDependencyUse {
        index: u32,
    },
    ProjectionCountMismatch {
        kind: ImportedCoreUseKind,
        export: usize,
        local: usize,
    },
    SelectionCountMismatch {
        kind: ImportedCoreUseKind,
        hir: usize,
        selected: usize,
    },
    ProjectionMismatch {
        kind: ImportedCoreUseKind,
        index: u32,
    },
    ForeignImportedCoreUse {
        kind: ImportedCoreUseKind,
        index: u32,
    },
    DuplicateImportedCoreBinding {
        kind: ImportedCoreUseKind,
        index: u32,
    },
}

impl fmt::Display for OrdinaryHirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot seal ordinary imported-core HIR: {self:?}"
        )
    }
}

impl std::error::Error for OrdinaryHirOutputError {}
