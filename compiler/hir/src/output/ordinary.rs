use std::collections::HashSet;
use std::fmt;

use crate::{CoreProtocols, LocalConcreteMaterializationContract, concrete};

/// Ordinary HIR product with the committed dependency selections and source
/// binding routes needed by subsequent interface and machine-IR production.
pub struct OrdinaryHirOutput {
    output: crate::Output,
    imported_dependencies: crate::SelectedImportedDependencySet,
    binding_witness_uses: Vec<crate::ExternalHirBindingWitnessUse>,
    concrete_dependency_witness_uses: Vec<crate::ExternalHirBindingWitnessUse>,
}

impl OrdinaryHirOutput {
    pub fn try_new(
        output: crate::Output,
        imported_dependencies: crate::SelectedImportedDependencySet,
        mut binding_witness_uses: Vec<crate::ExternalHirBindingWitnessUse>,
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
        let concrete_dependency_witness_uses =
            concrete_dependency_witness_uses(&imported_dependencies);
        binding_witness_uses.sort_unstable();
        binding_witness_uses.dedup();

        Ok(Self {
            output,
            imported_dependencies,
            binding_witness_uses,
            concrete_dependency_witness_uses,
        })
    }

    pub const fn output(&self) -> &crate::Output {
        &self.output
    }

    pub const fn imported_dependencies(&self) -> &crate::SelectedImportedDependencySet {
        &self.imported_dependencies
    }

    /// Source-name proofs retained by HIR lowering for export-surface uses
    /// whose route cannot be reconstructed from the projected interface.
    pub fn binding_witness_uses(&self) -> &[crate::ExternalHirBindingWitnessUse] {
        &self.binding_witness_uses
    }

    /// Canonical source-name proofs for every committed ordinary-dependency
    /// HIR use. These are derived from the winner-only selection transaction,
    /// never reconstructed from transient local-concrete nodes.
    pub fn concrete_dependency_witness_uses(&self) -> &[crate::ExternalHirBindingWitnessUse] {
        &self.concrete_dependency_witness_uses
    }

    pub fn into_parts(self) -> (crate::Output, crate::SelectedImportedDependencySet) {
        (self.output, self.imported_dependencies)
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
    for alias in selected.type_aliases() {
        let target = crate::ExternalHirTargetV1::TypeAlias(alias.interface().alias());
        append_concrete_dependency_witnesses(&mut uses, target, alias.binding());
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
