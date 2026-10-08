use std::collections::HashSet;
use std::fmt;

use crate::concrete;

mod callables;
mod equality;
mod occurrences;
mod witnesses;
pub use callables::ExecutableDependencyCallableUse;
pub use occurrences::{
    CommittedDependencyCallOccurrence, CommittedDependencyCallTarget,
    DependencyCallOccurrenceError, DependencyCallOrigin,
};

/// HIR product with the committed dependency selections and source
/// binding routes needed by subsequent interface and machine-IR production.
pub struct DependencyHirOutput {
    output: crate::Output,
    imported_dependencies: crate::SelectedImportedDependencySet,
    binding_witness_uses: Vec<crate::ExternalHirBindingWitnessUse>,
    concrete_dependency_witness_uses: Vec<crate::ExternalHirBindingWitnessUse>,
    executable_callables: Vec<concrete::ImportedDependencyCallableUseId>,
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
        let mut executable_callables = Vec::new();
        let concrete_dependency_witness_uses =
            witnesses::collect(&output, &imported_dependencies, &mut executable_callables)
                .map_err(DependencyHirOutputError::CallOccurrence)?;
        local
            .visit_executable_expressions(|occurrence| {
                let callable = if let Some(concrete::LiteralPatternEquality::Ordinary {
                    equals: concrete::CallableTarget::Imported(callable),
                }) = occurrence.literal_equality
                {
                    callable
                } else {
                    match &occurrence.expression.kind {
                        concrete::ExprKind::FunctionAddress(
                            concrete::CallableTarget::Imported(callable),
                        ) => *callable,
                        concrete::ExprKind::ClassInitializerCall {
                            initializer: concrete::ClassInitializerTarget::Imported(callable),
                            ..
                        } => *callable,
                        concrete::ExprKind::CallableReference(id) => {
                            match local.callable_references[*id].target.callee() {
                                Some(concrete::CallableTarget::Imported(callable)) => callable,
                                Some(
                                    concrete::CallableTarget::Local(_)
                                    | concrete::CallableTarget::DerivedEquality(_),
                                )
                                | None => return Ok(()),
                            }
                        }
                        _ => return Ok(()),
                    }
                };
                if callable.into_raw().into_u32() as usize
                    >= local.imported_dependency_callables.len()
                {
                    return Err(DependencyCallOccurrenceError::MissingUse(
                        occurrence.position,
                    ));
                }
                executable_callables.push(callable);
                Ok(())
            })
            .map_err(|error| {
                DependencyHirOutputError::CallOccurrence(match error {
                    concrete::ExecutableExpressionVisitError::Structure(error) => {
                        DependencyCallOccurrenceError::Structure(error)
                    }
                    concrete::ExecutableExpressionVisitError::Visitor(error) => error,
                })
            })?;
        for method in local.classes.iter().flat_map(|(_, class)| &class.methods) {
            if let concrete::ClassMethod::Imported { callable, .. } = *method {
                if callable.into_raw().into_u32() as usize
                    >= local.imported_dependency_callables.len()
                {
                    return Err(DependencyHirOutputError::MissingDispatchUse {
                        index: callable.into_raw().into_u32(),
                    });
                }
                executable_callables.push(callable);
            }
        }
        let local_value_dispatch = |origin: &crate::HirNominalIdentity| {
            !matches!(
                origin,
                crate::HirNominalIdentity::Source(crate::HirSourceNominalIdentity::Concrete(record))
                    if record.key().origin() != local.cone
            )
        };
        for implementation in local
            .classes
            .iter()
            .flat_map(|(_, class)| &class.interface_implementations)
            .chain(
                local
                    .structs
                    .iter()
                    .filter(|(_, value)| local_value_dispatch(&value.origin))
                    .flat_map(|(_, value)| &value.interface_implementations),
            )
            .chain(
                local
                    .enums
                    .iter()
                    .filter(|(_, value)| local_value_dispatch(&value.origin))
                    .flat_map(|(_, value)| &value.interface_implementations),
            )
        {
            for method in &implementation.methods {
                let callable = match method.target {
                    concrete::InterfaceImplementationTarget::Imported(callable)
                    | concrete::InterfaceImplementationTarget::ImportedAbstract {
                        declaration: callable,
                    } => callable,
                    concrete::InterfaceImplementationTarget::Method(_)
                    | concrete::InterfaceImplementationTarget::ImportedDerivedEquality(_)
                    | concrete::InterfaceImplementationTarget::Abstract { .. } => continue,
                };
                if callable.into_raw().into_u32() as usize
                    >= local.imported_dependency_callables.len()
                {
                    return Err(DependencyHirOutputError::MissingDispatchUse {
                        index: callable.into_raw().into_u32(),
                    });
                }
                executable_callables.push(callable);
            }
        }
        executable_callables.sort_unstable();
        executable_callables.dedup();
        binding_witness_uses.sort_unstable();
        binding_witness_uses.dedup();

        Ok(Self {
            output,
            imported_dependencies,
            binding_witness_uses,
            concrete_dependency_witness_uses,
            executable_callables,
        })
    }

    pub const fn output(&self) -> &crate::Output {
        &self.output
    }

    pub const fn imported_dependencies(&self) -> &crate::SelectedImportedDependencySet {
        &self.imported_dependencies
    }

    /// Source lookup routes retained by HIR lowering for export-surface uses
    /// whose route cannot be reconstructed from the projected interface.
    pub fn binding_witness_uses(&self) -> &[crate::ExternalHirBindingWitnessUse] {
        &self.binding_witness_uses
    }

    /// Canonical source lookup routes for concrete dependency uses.
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
        if !references.insert((reference, export_use.dispatch())) {
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
    MissingDispatchUse {
        index: u32,
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
