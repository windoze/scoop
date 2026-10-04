use super::*;

impl Lowerer {
    fn is_context_reference_type(&self, ty: TypeId) -> bool {
        if let hir::Type::Param(id) = self.types[ty] {
            return self.type_params_in_scope.iter().any(|parameter| parameter.id == id &&
                (parameter.kind() == hir::TypeParamKind::Ref ||
                 matches!(&parameter.bounds, hir::TypeParamBounds::Nominal(bounds) if bounds.class.is_some())));
        }
        self.is_ref_ty(ty)
    }
    pub(crate) fn resolve_context_parameters(
        &mut self,
        declaration: &ast::FunctionDecl,
    ) -> Vec<hir::ContextParameter> {
        let mut parameters: Vec<hir::ContextParameter> = Vec::new();
        let mut names = declaration
            .params
            .iter()
            .map(|parameter| parameter.name.text.clone())
            .collect::<std::collections::HashSet<_>>();
        for parameter in &declaration.context_parameters {
            let Some(ty) = self.resolve_type_ref(&parameter.ty) else {
                continue;
            };
            if !self.is_context_reference_type(ty) {
                self.error(
                    parameter.ty.span,
                    format!(
                        "context key must be a non-null managed reference type, found {}",
                        self.type_name(ty)
                    ),
                );
            }
            if parameters
                .iter()
                .any(|earlier| self.types_equal(earlier.ty, ty))
            {
                self.error(
                    parameter.ty.span,
                    format!("duplicate context key `{}`", self.type_name(ty)),
                );
            }
            let label = match &parameter.label {
                ast::ContextParameterLabel::Named(name) => {
                    if !names.insert(name.text.clone()) {
                        self.error(
                            name.span,
                            format!("duplicate context or value parameter `{}`", name.text),
                        );
                    }
                    hir::ContextParameterLabel::Named(name.text.clone())
                }
                ast::ContextParameterLabel::Unnamed(_) => hir::ContextParameterLabel::Unnamed,
            };
            parameters.push(hir::ContextParameter {
                label,
                ty,
                span: parameter.span,
            });
        }
        if !parameters.is_empty() {
            for annotation in &declaration.annotations {
                if matches!(
                    annotation.name.text.as_str(),
                    "Extern" | "Intrinsic" | "NoGC"
                ) {
                    self.error(
                        annotation.span,
                        format!(
                            "contextual declarations cannot be annotated with `@{}`",
                            annotation.name.text
                        ),
                    );
                }
            }
        }
        parameters
    }

    pub(crate) fn lower_context_entry(&mut self, function: hir::FunctionId) -> Vec<hir::Statement> {
        let parameters = self.functions[function].context_parameters.clone();
        if !parameters.is_empty() {
            if let Err(error) = self.prepare_missing_context_exception_type() {
                self.error(
                    parameters[0].span,
                    format!("cannot materialize context exception: {error:?}"),
                );
            }
            self.option_type(self.string);
        }
        parameters
            .into_iter()
            .enumerate()
            .map(|(index, parameter)| {
                let lookup = hir::Expr {
                    kind: hir::ExprKind::ContextLookup(hir::ContextRequirementRef {
                        declaration: hir::ContextRequirementOwner::Source(function),
                        diagnostic: hir::ContextDiagnostic {
                            declaration: self.functions[function].name.clone(),
                            label: parameter.label.clone(),
                        },
                        parameter: hir::ContextParameterIndex(
                            index.try_into().expect("context parameter index fits u32"),
                        ),
                    }),
                    ty: parameter.ty,
                    span: parameter.span,
                    origin: self.expression_origin(parameter.span),
                };
                let kind = match parameter.label {
                    hir::ContextParameterLabel::Named(name) => {
                        let local = self.alloc_declared_local(
                            name.clone(),
                            parameter.ty,
                            false,
                            parameter.span,
                        );
                        self.scopes.declare(name, local);
                        hir::StatementKind::ValDecl {
                            pattern: hir::Pattern::Binding { local },
                            init: lookup,
                        }
                    }
                    hir::ContextParameterLabel::Unnamed => hir::StatementKind::Expr(lookup),
                };
                hir::Statement {
                    kind,
                    span: parameter.span,
                }
            })
            .collect()
    }

    pub(super) fn lower_context_scope(
        &mut self,
        source_value: &ast::Expr,
        source_body: &ast::Block,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let value = self.lower_expr(source_value, sink, None)?;
        if !self.is_context_reference_type(value.ty) {
            self.error(
                source_value.span(),
                format!(
                    "context binding must have a non-null managed reference type, found {}",
                    self.type_name(value.ty)
                ),
            );
            return None;
        }
        let mut body = self.lower_value_block(source_body, expected)?;
        let result = self.finish_control_value("context", span, expected, &mut [&mut body])?;
        sink.push(hir::Statement {
            kind: hir::StatementKind::ContextScope {
                value,
                body: body.statements,
            },
            span,
        });
        Some(result)
    }
}
