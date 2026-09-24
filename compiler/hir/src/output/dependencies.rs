use std::collections::HashSet;
use std::fmt;

use crate::concrete;

mod callables;
mod occurrences;
mod witnesses;
pub use callables::ExecutableDependencyCallableUse;
pub use occurrences::{
    CommittedDependencyCallOccurrence, DependencyCallOccurrenceError, DependencyCallOrigin,
};

/// HIR product with the committed dependency selections and source
/// binding routes needed by subsequent interface and machine-IR production.
pub struct DependencyHirOutput {
    output: crate::Output,
    imported_dependencies: crate::SelectedImportedDependencySet,
    binding_witness_uses: Vec<crate::ExternalHirBindingWitnessUse>,
    concrete_dependency_witness_uses: Vec<crate::ExternalHirBindingWitnessUse>,
}

impl DependencyHirOutput {
    pub fn try_new(
        output: crate::Output,
        imported_dependencies: crate::SelectedImportedDependencySet,
        mut binding_witness_uses: Vec<crate::ExternalHirBindingWitnessUse>,
    ) -> Result<Self, DependencyHirOutputError> {
        if imported_dependencies.consumer() != output.export.module().cone {
            return Err(DependencyHirOutputError::DependencyConsumerMismatch {
                output: output.export.module().cone,
                selected: imported_dependencies.consumer(),
            });
        }
        let export = output.export.module();
        let local = output.local.module();
        validate_imported_dependency_projection(export, local, &imported_dependencies)?;
        let concrete_dependency_witness_uses = witnesses::collect(&output, &imported_dependencies)
            .map_err(DependencyHirOutputError::CallOccurrence)?;
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

    /// Canonical source-name proofs for concrete ordinary-dependency uses.
    /// Callable routes come from actual executable nodes; source-only
    /// default references retain their separate source metadata roles.
    pub fn concrete_dependency_witness_uses(&self) -> &[crate::ExternalHirBindingWitnessUse] {
        &self.concrete_dependency_witness_uses
    }

    pub fn into_parts(self) -> (crate::Output, crate::SelectedImportedDependencySet) {
        (self.output, self.imported_dependencies)
    }
}

fn validate_imported_dependency_projection(
    export: &crate::ExportHir,
    local: &concrete::Module,
    selected: &crate::SelectedImportedDependencySet,
) -> Result<(), DependencyHirOutputError> {
    let export_count = export.imported_dependency_callables.len();
    let local_count = local.imported_dependency_callables.len();
    if export_count != local_count {
        return Err(
            DependencyHirOutputError::DependencyProjectionCountMismatch {
                export: export_count,
                local: local_count,
            },
        );
    }
    if export_count != selected.callable_count() {
        return Err(DependencyHirOutputError::DependencySelectionCountMismatch {
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
            return Err(DependencyHirOutputError::DependencyProjectionMismatch { index });
        }
        let reference = export_use.reference();
        if selected.resolve_callable(reference).is_none() {
            return Err(DependencyHirOutputError::ForeignImportedDependencyUse { index });
        }
        if !references.insert(reference) {
            return Err(DependencyHirOutputError::DuplicateImportedDependencyUse { index });
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyHirOutputError {
    CallOccurrence(DependencyCallOccurrenceError),
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

impl fmt::Display for DependencyHirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot seal dependency-aware HIR: {self:?}")
    }
}

impl std::error::Error for DependencyHirOutputError {}
