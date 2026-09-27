//! Actual call sites borrow the winning route from their executable HIR node.

use scoop_wire::{WireError, WirePath};

use super::*;
use crate::{
    ConcreteExpressionOrigin, DirectImportedTargetBinding, HirDefinitionSourceProjectionError,
    SelectedImportedDependencyCallable,
};
use concrete::{ExecutableExpressionOccurrence, ExecutableExpressionPosition};

#[derive(Clone, Copy, Debug)]
pub enum CommittedDependencyCallTarget<'a> {
    Direct {
        callee: concrete::ImportedDependencyCallableUseId,
        callable: &'a SelectedImportedDependencyCallable,
    },
    Application(&'a concrete::CallableApplicationRecord),
}

#[derive(Clone, Copy, Debug)]
pub struct CommittedDependencyCallOccurrence<'a> {
    occurrence: ExecutableExpressionOccurrence<'a>,
    target: CommittedDependencyCallTarget<'a>,
    binding: Option<&'a DirectImportedTargetBinding>,
    arguments: &'a [concrete::Expr],
    receiver: crate::SourceCallReceiver<concrete::TypeId>,
}

impl<'a> CommittedDependencyCallOccurrence<'a> {
    pub(super) fn validate_origin(
        self,
        export: &crate::ExportHir,
    ) -> Result<(), DependencyCallOccurrenceError> {
        validate_origins(export, self.occurrence)
    }

    pub const fn target(self) -> CommittedDependencyCallTarget<'a> {
        self.target
    }

    pub fn declaration(self) -> scoop_identity::CallableTemplateOrigin {
        match self.target {
            CommittedDependencyCallTarget::Direct { callable, .. } => {
                callable.interface().declaration()
            }
            CommittedDependencyCallTarget::Application(application) => application.key().origin(),
        }
    }

    pub fn instantiation(self) -> crate::HirDependencyCallInstantiationV1 {
        match self.target {
            CommittedDependencyCallTarget::Direct { .. } => {
                crate::HirDependencyCallInstantiationV1::Direct
            }
            CommittedDependencyCallTarget::Application(application) => {
                crate::HirDependencyCallInstantiationV1::Application(application.id())
            }
        }
    }

    pub const fn position(self) -> ExecutableExpressionPosition {
        self.occurrence.position
    }
    pub const fn origin(self) -> ConcreteExpressionOrigin {
        self.occurrence.expression.origin
    }
    pub const fn binding(self) -> Option<&'a DirectImportedTargetBinding> {
        self.binding
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
            let position = occurrence.position;
            let (target, binding, args, receiver) = match &occurrence.expression.kind {
                concrete::ExprKind::ImportedDependencyCall {
                    callee,
                    binding,
                    args,
                    receiver,
                } => {
                    if callee.into_raw().into_u32() as usize
                        >= local.imported_dependency_callables.len()
                    {
                        return Err(DependencyCallOccurrenceError::MissingUse(position));
                    }
                    let reference = local.imported_dependency_callables[*callee].reference();
                    let callable = selected
                        .resolve_callable(reference)
                        .ok_or(DependencyCallOccurrenceError::UnselectedUse(position))?;
                    (
                        CommittedDependencyCallTarget::Direct {
                            callee: *callee,
                            callable,
                        },
                        binding,
                        args,
                        receiver,
                    )
                }
                concrete::ExprKind::ImportedGenericCall {
                    callee,
                    binding,
                    args,
                    receiver,
                } => {
                    if callee.into_raw().into_u32() as usize >= local.functions.len() {
                        return Err(DependencyCallOccurrenceError::MissingUse(position));
                    }
                    let materialization = local.functions[*callee].materialization;
                    let scoop_identity::CallableMaterializationContext::Application(id) =
                        materialization.context()
                    else {
                        return Err(DependencyCallOccurrenceError::InvalidApplication(position));
                    };
                    let application = local
                        .callable_applications
                        .get(id)
                        .ok_or(DependencyCallOccurrenceError::InvalidApplication(position))?;
                    if !matches!((materialization.template(), application.key().origin()),
                        (scoop_identity::CallableTemplateOwner::GenericFunction(expected),
                         scoop_identity::CallableTemplateOrigin::GenericFunction(actual)) if expected == actual) {
                        return Err(DependencyCallOccurrenceError::InvalidApplication(position));
                    }
                    (
                        CommittedDependencyCallTarget::Application(application),
                        binding,
                        args,
                        receiver,
                    )
                }
                _ => return Ok(()),
            };
            visitor(CommittedDependencyCallOccurrence {
                occurrence,
                target,
                binding: binding.as_deref(),
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
            let template = match occurrence.position.root.template() {
                scoop_identity::CallableTemplateOwner::GenericFunction(identity) => export
                    .imported_generic_templates
                    .iter()
                    .map(|(_, template)| template)
                    .find(|template| template.declaration == identity),
                _ => None,
            };
            if template.is_none_or(|template| {
                export.source_files[template.origin.file as usize].identity
                    != *projected.origin().source()
            }) {
                return Err(DependencyCallOccurrenceError::ForeignEvaluation(
                    occurrence.position,
                ));
            }
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
    InvalidApplication(ExecutableExpressionPosition),
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
