//! Actual call sites borrow the winning route from their executable HIR node.

use scoop_wire::{WireError, WirePath};

use super::*;
use crate::{
    ConcreteExpressionOrigin, DirectImportedTargetBinding, HirDefinitionSourceProjectionError,
    SelectedImportedDependencyCallable,
};
use concrete::{ExecutableExpressionOccurrence, ExecutableExpressionPosition};

#[derive(Clone, Copy, Debug)]
pub struct CommittedDependencyCallOccurrence<'a> {
    occurrence: ExecutableExpressionOccurrence<'a>,
    callee: concrete::ImportedDependencyCallableUseId,
    binding: &'a DirectImportedTargetBinding,
    callable: &'a SelectedImportedDependencyCallable,
    arguments: &'a [concrete::Expr],
    receiver: crate::SourceCallReceiver<concrete::TypeId>,
}

impl<'a> CommittedDependencyCallOccurrence<'a> {
    pub const fn callee(self) -> concrete::ImportedDependencyCallableUseId {
        self.callee
    }

    pub const fn position(self) -> ExecutableExpressionPosition {
        self.occurrence.position
    }
    pub const fn origin(self) -> ConcreteExpressionOrigin {
        self.occurrence.expression.origin
    }
    pub const fn binding(self) -> &'a DirectImportedTargetBinding {
        self.binding
    }
    pub const fn callable(self) -> &'a SelectedImportedDependencyCallable {
        self.callable
    }
    pub const fn arguments(self) -> &'a [concrete::Expr] {
        self.arguments
    }
    pub const fn result_type(self) -> concrete::TypeId {
        self.occurrence.expression.ty
    }
    pub const fn receiver(self) -> crate::SourceCallReceiver<concrete::TypeId> {
        self.receiver
    }
}

impl DependencyHirOutput {
    /// Each actual use remains distinct even when its target, binding or
    /// default definition is shared with another occurrence.
    pub fn committed_dependency_call_occurrences(
        &self,
    ) -> Result<Vec<CommittedDependencyCallOccurrence<'_>>, DependencyCallOccurrenceError> {
        let mut occurrences = Vec::new();
        visit(&self.output, &self.imported_dependencies, |occurrence| {
            scoop_wire::allocation::try_reserve(&mut occurrences, 1, &WirePath::root())?;
            occurrences.push(occurrence);
            Ok(())
        })?;
        Ok(occurrences)
    }
}

pub(super) fn visit<'a>(
    output: &'a crate::Output,
    selected: &'a crate::SelectedImportedDependencySet,

    mut visitor: impl FnMut(
        CommittedDependencyCallOccurrence<'a>,
    ) -> Result<(), DependencyCallOccurrenceError>,
) -> Result<(), DependencyCallOccurrenceError> {
    let local = output.local.module();
    local
        .visit_executable_expressions(|occurrence| {
            let concrete::ExprKind::ImportedDependencyCall {
                callee,
                binding,
                args,
                receiver,
            } = &occurrence.expression.kind
            else {
                return Ok(());
            };
            let position = occurrence.position;
            if callee.into_raw().into_u32() as usize >= local.imported_dependency_callables.len() {
                return Err(DependencyCallOccurrenceError::MissingUse(position));
            }
            let reference = local.imported_dependency_callables[*callee].reference();

            let callable = selected
                .resolve_callable(reference)
                .ok_or(DependencyCallOccurrenceError::UnselectedUse(position))?;
            if !binding.is_selected_subset(callable.binding())? {
                return Err(DependencyCallOccurrenceError::Binding(position));
            }
            validate_origins(output.export.module(), occurrence)?;
            visitor(CommittedDependencyCallOccurrence {
                occurrence,
                callee: *callee,
                binding,
                callable,
                arguments: args,
                receiver: *receiver,
            })
        })
        .map_err(|error| match error {
            concrete::ExecutableExpressionVisitError::Structure(error) => {
                DependencyCallOccurrenceError::Structure(error)
            }
            concrete::ExecutableExpressionVisitError::Visitor(error) => error,
        })
}

fn validate_origins(
    export: &crate::ExportHir,
    occurrence: ExecutableExpressionOccurrence<'_>,
) -> Result<(), DependencyCallOccurrenceError> {
    let origin = occurrence.expression.origin;
    let evaluation = crate::DefinitionOrigin {
        provider: origin.evaluation.provider,
        file: origin.evaluation.file,
        span: origin.evaluation.span,
        context: origin.evaluation.context,
    };
    for (side, source) in [
        (DependencyCallOrigin::Definition, origin.definition),
        (DependencyCallOrigin::Evaluation, evaluation),
    ] {
        let projected =
            crate::production::project_definition_source(export, source).map_err(|source| {
                DependencyCallOccurrenceError::Origin {
                    position: occurrence.position,
                    side,
                    source,
                }
            })?;
        if side == DependencyCallOrigin::Evaluation
            && projected.origin().source().cone() != export.cone
        {
            return Err(DependencyCallOccurrenceError::ForeignEvaluation(
                occurrence.position,
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyCallOrigin {
    Definition,
    Evaluation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyCallOccurrenceError {
    Resource(WireError),
    Structure(concrete::ExecutableExpressionStructureError),
    MissingUse(ExecutableExpressionPosition),
    UnselectedUse(ExecutableExpressionPosition),
    Binding(ExecutableExpressionPosition),
    ForeignEvaluation(ExecutableExpressionPosition),
    Origin {
        position: ExecutableExpressionPosition,
        side: DependencyCallOrigin,
        source: HirDefinitionSourceProjectionError,
    },
}

impl From<WireError> for DependencyCallOccurrenceError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for DependencyCallOccurrenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid committed dependency call: {self:?}")
    }
}

impl std::error::Error for DependencyCallOccurrenceError {}
