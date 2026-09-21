use scoop_hir as hir;
use std::collections::HashMap;

use crate::defaults::{DefaultArgumentSource, DefaultExprTemplateRef};
use crate::{Lowerer, Type};

mod closures;
mod entities;
mod expressions;
mod iteration;
mod patterns;
mod statements;

struct InstantiationContext {
    bindings: Vec<(hir::TypeParamId, hir::TypeId)>,
    locals: Vec<hir::Expr>,
    captures: HashMap<hir::BindingId, hir::Expr>,
    local_functions: HashMap<hir::LocalFunctionId, hir::LocalFunctionId>,
    loop_targets: Vec<(hir::LoopId, hir::LoopId)>,
    evaluation: InstantiationEvaluation,
}

#[derive(Clone, Copy)]
enum InstantiationEvaluation {
    Template,
    Concrete(hir::EvaluationOrigin),
}

impl Lowerer {
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
                (template.body, template.captures, bindings.to_vec())
            }
            DefaultExprTemplateRef::Export(source) => {
                let source = self.export_default_sources[source].clone();
                let template = self.export_default_exprs[source.expression].clone();
                assert_eq!(template.type_parameters.len(), source.type_arguments.len());
                let arguments = source
                    .type_arguments
                    .into_iter()
                    .map(|argument| self.instantiate_method_ty(argument, bindings))
                    .collect::<Vec<_>>();
                let template_bindings = template
                    .type_parameters
                    .iter()
                    .copied()
                    .zip(arguments)
                    .collect();
                (template, Vec::new(), template_bindings)
            }
        };
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
            local_functions: HashMap::new(),
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
        Some(self.instantiate_default_expr(&template.value, &mut context))
    }
}

impl InstantiationContext {
    fn local_function(&self, source: hir::LocalFunctionId) -> hir::LocalFunctionId {
        // A call to a declaration outside this default retains its source descriptor.
        self.local_functions.get(&source).copied().unwrap_or(source)
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
