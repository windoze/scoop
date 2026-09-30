use std::collections::BTreeMap;
use std::fmt;

use hir::ImportedCallableSource;
use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::LocalValueSelector;

use super::plan::PreparedImportedDefault;
use crate::Lowerer;
use crate::expr::MemberCallKind;
use crate::expr::imported_origins::ImportedDefinitionOriginError;

mod arrays;
mod callable;
mod capture_bindings;
mod closures;
mod constructors;
mod definition;
mod errors;
mod expressions;
use errors::ImportedDefaultMaterializationError;
mod delegates;
mod methods;
mod patterns;
mod pointers;
mod references;
mod statements;

struct ImportedDefaultContext<'a> {
    owner: ImportedTemplateSource<'a>,
    bindings: &'a crate::imported_core::ImportedTypeBindings,
    locals: BTreeMap<LocalValueSelector, hir::Expr>,
    local_bindings: BTreeMap<LocalValueSelector, hir::BindingId>,
    lexical_arguments: Vec<hir::TypeId>,
    captures: &'a [hir::BindingId],
    loop_targets: Vec<hir::LoopId>,
    parent: scoop_identity::CallableTemplateOwner,
}

enum ImportedTemplateSource<'a> {
    Default(&'a dyn hir::ImportedCallableSource),
    Callable(&'a crate::imported_generics::PreparedImportedCallableSource),
}

impl ImportedTemplateSource<'_> {
    fn source_location(
        &self,
        source: &scoop_identity::SourceIdentity,
        context: scoop_identity::PersistentSourceContextId,
    ) -> Option<hir::ImportedDependencyDefinitionSource<'_>> {
        match self {
            Self::Default(owner) => owner.source_location(source, context),
            Self::Callable(owner) => owner.source_location(source, context),
        }
    }
    fn definition_source(
        &self,
        source: &hir::ExportDefinitionSourceV1,
    ) -> Option<hir::ImportedDependencyDefinitionSource<'_>> {
        match self {
            Self::Default(owner) => owner.definition_source(source),
            Self::Callable(owner) => owner.definition_source(source),
        }
    }
}

impl Lowerer {
    pub(crate) fn materialize_imported_default(
        &mut self,
        prepared: &PreparedImportedDefault,
        receiver: Option<&hir::Expr>,
        value_parameters: &[hir::Expr],
        call_span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Result<hir::Expr, ImportedDefaultMaterializationError> {
        let bindings = prepared
            .expression
            .type_parameters
            .iter()
            .copied()
            .zip(prepared.arguments.iter().copied())
            .collect::<Vec<_>>();
        let receiver = if let Some(parameter) = prepared.expression.receiver {
            let value = receiver
                .cloned()
                .ok_or(ImportedDefaultMaterializationError::MissingReceiver)?;
            let ty = self.instantiate_method_ty(parameter.ty, &bindings);
            Some(self.adapt_to(value, ty))
        } else {
            None
        };
        Ok(self.instantiate_default_expression(
            &prepared.expression,
            Vec::new(),
            bindings,
            receiver.as_ref(),
            value_parameters,
            call_span,
            sink,
        ))
    }

    fn materialize_imported_default_expressions(
        &mut self,
        expressions: &[hir::DefaultExpressionV1],
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<Vec<hir::Expr>, ImportedDefaultMaterializationError> {
        expressions
            .iter()
            .map(|expression| self.materialize_imported_default_expression(expression, context))
            .collect()
    }

    fn imported_default_call_kind(
        &mut self,
        callee: scoop_identity::CallableTemplateOrigin,
        args: Vec<hir::Expr>,
        receiver: hir::SourceCallReceiver<hir::TypeId>,
        kind: MemberCallKind,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        let callee = self.imported_default_callable_target(callee, kind)?;
        Ok(hir::ExprKind::Call {
            callee: hir::CallableTarget::Dependency(callee),
            binding: None,
            args,
            receiver,
        })
    }

    fn imported_default_callable_target(
        &mut self,
        callee: scoop_identity::CallableTemplateOrigin,
        kind: MemberCallKind,
    ) -> Result<hir::ImportedDependencyCallableUseId, ImportedDefaultMaterializationError> {
        let candidate = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.callable_declaration(callee).ok())
            .ok_or(ImportedDefaultMaterializationError::MissingCallable(callee))?;
        if candidate.interface().effects().implementation() != hir::CallableImplementationV1::Scoop
        {
            return Err(ImportedDefaultMaterializationError::Plan(
                crate::imported_capabilities::ImportedCapabilityRequirement::Native
                    .diagnostic("dependency default native call"),
            ));
        }
        self.select_imported_callable_declaration_use_with_kind(candidate, kind)
            .map_err(|error| {
                ImportedDefaultMaterializationError::DependencySelection(error.to_string())
            })
    }

    fn imported_default_definition_origin(
        &mut self,
        source: &hir::ExportDefinitionSourceV1,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::DefinitionOrigin, ImportedDefaultMaterializationError> {
        let imported = context.owner.definition_source(source).ok_or_else(|| {
            ImportedDefinitionOriginError::MissingSource {
                context: source.origin().context(),
            }
        })?;
        self.import_dependency_definition_origin(source, imported)
            .map_err(Into::into)
    }
}
