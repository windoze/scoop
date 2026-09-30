use scoop_hir as hir;
use std::collections::HashMap;

use crate::defaults::{DefaultArgumentSource, DefaultExprTemplateRef};
use crate::{Lowerer, Type};

mod closures;
mod entities;
mod expressions;
mod patterns;
mod signatures;
mod statements;

struct InstantiationContext {
    bindings: Vec<(hir::TypeParamId, hir::TypeId)>,
    locals: Vec<hir::Expr>,
    captures: HashMap<hir::BindingId, hir::Expr>,
    loop_targets: Vec<(hir::LoopId, hir::LoopId)>,
    evaluation: InstantiationEvaluation,
    statement_span: scoop_ast::Span,
}

#[derive(Clone, Copy)]
enum InstantiationEvaluation {
    Template,
    Concrete(hir::EvaluationOrigin),
}

impl Lowerer {
    fn instantiate_delegate_reference(
        &mut self,
        source: &hir::GenericDelegateReference,
        context: &InstantiationContext,
    ) -> hir::GenericDelegateReference {
        let arguments = source
            .arguments
            .iter()
            .map(|argument| self.instantiate_method_ty(*argument, &context.bindings))
            .collect();
        hir::GenericDelegateReference {
            template: source.template,
            arguments: hir::NonEmptyVec::from_vec(arguments)
                .expect("type substitution preserves nonempty delegate arguments"),
        }
    }

    pub(crate) fn instantiate_default(
        &mut self,
        source: DefaultArgumentSource,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
        receiver: Option<&hir::Expr>,
        value_parameters: &[hir::Expr],
        call_span: scoop_ast::Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let template = self.resolve_default_argument(source, call_span)?;
        let (template, captures, template_bindings) = match template {
            DefaultExprTemplateRef::Local(template) => {
                let template = self.local_default_exprs[template].clone();
                (
                    template.body.expression,
                    template.captures,
                    bindings.to_vec(),
                )
            }
            DefaultExprTemplateRef::Export(source) => {
                let source = self.export_default_sources[source].clone();
                let (expression, type_arguments) = match source {
                    hir::ExportDefaultSource::Declared {
                        expression,
                        type_arguments,
                    } => (expression, type_arguments),
                    hir::ExportDefaultSource::Imported {
                        template,
                        type_arguments,
                    } => {
                        let arguments = type_arguments
                            .into_iter()
                            .map(|argument| self.instantiate_method_ty(argument, bindings))
                            .collect();
                        return self.instantiate_inherited_dependency_default(
                            &template,
                            arguments,
                            receiver,
                            value_parameters,
                            call_span,
                            sink,
                        );
                    }
                };
                let template = self.export_default_exprs[expression].clone();
                assert_eq!(template.type_parameters.len(), type_arguments.len());
                let arguments = type_arguments
                    .into_iter()
                    .map(|argument| self.instantiate_method_ty(argument, bindings))
                    .collect::<Vec<_>>();
                let template_bindings = template
                    .type_parameters
                    .iter()
                    .copied()
                    .zip(arguments)
                    .collect();
                (template.expression, Vec::new(), template_bindings)
            }
        };
        Some(self.instantiate_default_expression(
            &template,
            captures,
            template_bindings,
            receiver,
            value_parameters,
            call_span,
            sink,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn instantiate_default_expression(
        &mut self,
        template: &hir::DefaultExpression,
        captures: Vec<hir::Capture>,
        template_bindings: Vec<(hir::TypeParamId, hir::TypeId)>,
        receiver: Option<&hir::Expr>,
        value_parameters: &[hir::Expr],
        call_span: scoop_ast::Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let mut mapped = vec![None; template.locals.len()];
        if let Some(source) = template.receiver {
            mapped[arena_index(source.local)] = Some(
                receiver
                    .expect("a receiver default is instantiated with its receiver")
                    .clone(),
            );
        }
        for source in &template.value_parameters {
            mapped[arena_index(source.local)] =
                Some(value_parameters[source.position as usize].clone());
        }
        for (source_id, source) in template.locals.iter() {
            if mapped[arena_index(source_id)].is_some() {
                continue;
            }
            let ty = self.instantiate_method_ty(source.ty, &template_bindings);
            let local = self.alloc_synthetic_local(
                source.name.clone(),
                ty,
                source.mutable,
                scoop_identity::SyntheticLocalRole::DefaultValue,
            );
            mapped[arena_index(source_id)] = Some(hir::Expr {
                kind: hir::ExprKind::Local(local),
                ty,
                span: template.origin.span,
                origin: hir::ExpressionOrigin::Definition(template.origin),
            });
        }
        let mut context = InstantiationContext {
            statement_span: call_span,
            bindings: template_bindings,
            locals: mapped
                .into_iter()
                .map(|local| local.expect("every default-template local is mapped"))
                .collect(),
            captures: captures
                .into_iter()
                .map(|capture| {
                    let value = self
                        .lower_capture_binding(
                            capture.binding,
                            &capture.name,
                            capture.first_use_span,
                        )
                        .unwrap_or(capture.source);
                    (capture.binding, value)
                })
                .collect(),
            loop_targets: Vec::new(),
            evaluation: if self.lowering_default_template {
                InstantiationEvaluation::Template
            } else {
                InstantiationEvaluation::Concrete(self.definition_origin(call_span).into())
            },
        };
        for statement in &template.statements {
            sink.push(self.instantiate_default_statement(statement, &mut context));
        }
        debug_assert!(context.loop_targets.is_empty());
        self.instantiate_default_expr(&template.value, &mut context)
    }
}

fn mapped_local(context: &InstantiationContext, source: hir::LocalId) -> hir::LocalId {
    let hir::ExprKind::Local(local) = context.locals[arena_index(source)].kind else {
        unreachable!("default template places only refer to mapped local temporaries")
    };
    local
}

fn instantiate_origin(
    source: hir::ExpressionOrigin,
    evaluation: InstantiationEvaluation,
) -> hir::ExpressionOrigin {
    match evaluation {
        InstantiationEvaluation::Template => hir::ExpressionOrigin::Definition(source.definition()),
        InstantiationEvaluation::Concrete(evaluation) => source.instantiate(evaluation),
    }
}

fn arena_index<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
