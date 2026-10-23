use super::*;

pub(crate) struct IterationHead {
    pub setup: Vec<hir::Statement>,
    pub condition_setup: Vec<hir::Statement>,
    pub condition: hir::Expr,
    pub element_setup: Vec<hir::Statement>,
    pub element: crate::patterns::BindingSubject,
}

impl Lowerer {
    pub(crate) fn lower_iteration_head(
        &mut self,
        iterable: &ast::Expr,
        span: Span,
    ) -> Option<IterationHead> {
        let mut statements = Vec::new();
        let source_init = self.lower_expr(iterable, &mut statements, None)?;
        let source_receiver =
            self.save_iteration_value("for.source", source_init, iterable.span(), &mut statements);
        let iterator_call = self.lower_named_call_on_receiver(
            source_receiver,
            &ast::Ident {
                text: "iterator".to_string(),
                span: iterable.span(),
            },
            CallSite {
                type_args: &[],
                args: &[],
                span: iterable.span(),
            },
            &mut statements,
            None,
            RequiredCallableModifiers {
                operator: Some(hir::OperatorKind::Iterator),
                ..Default::default()
            },
        )?;

        let applications = self.iteration_interface_applications(iterator_call.ty);
        let iterator_type = match applications.as_slice() {
            [application] => *application,
            [] => {
                let found = self.type_name(iterator_call.ty);
                self.error(
                    iterable.span(),
                    format!(
                        "iterator operator result type `{found}` does not implement `Iterator<T>`"
                    ),
                );
                return None;
            }
            _ => {
                let found = self.type_name(iterator_call.ty);
                self.error(
                    iterable.span(),
                    format!(
                        "iterator operator result type `{found}` implements multiple distinct `Iterator<T>` applications"
                    ),
                );
                return None;
            }
        };
        let arguments = match &self.types[iterator_type] {
            Type::Interface(application) => &self.interface_applications[*application].arguments,
            _ => unreachable!("iteration conformance names an exact interface application"),
        };
        let [element_type] = arguments.as_slice() else {
            unreachable!("the checked Iterator core has one type parameter")
        };
        let element_type = *element_type;
        let raw_iterator =
            self.save_iteration_value("for.iterator.result", iterator_call, span, &mut statements);
        let adapted = self.adapt_to(raw_iterator, iterator_type);
        let iterator = self.save_iteration_value("for.iterator", adapted, span, &mut statements);
        let option_type = self.option_type(element_type);
        let next_call = self.lower_iteration_next(iterator, option_type)?;
        let mut condition_setup = Vec::new();
        let next_result =
            self.save_iteration_value("for.next", next_call, span, &mut condition_setup);
        let origin = self.expression_origin(span);
        let cond = hir::Expr {
            kind: hir::ExprKind::IsSome(Box::new(next_result.clone())),
            ty: self.boolean,
            span,
            origin,
        };
        let element_init = hir::Expr {
            kind: hir::ExprKind::Unwrap {
                operand: Box::new(next_result),
                trap_on_none: false,
            },
            ty: element_type,
            span,
            origin,
        };
        let mut element_setup = Vec::new();
        let element =
            self.save_iteration_value("for.element", element_init, span, &mut element_setup);
        let hir::ExprKind::Local(local) = element.kind else {
            unreachable!("a saved iteration value is a local")
        };
        Some(IterationHead {
            setup: statements,
            condition_setup,
            condition: cond,
            element_setup,
            element: crate::patterns::BindingSubject {
                local,
                ty: element_type,
            },
        })
    }

    fn save_iteration_value(
        &mut self,
        name: &str,
        init: hir::Expr,
        span: Span,
        statements: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let ty = init.ty;
        let local = self.alloc_desugared_iterator_hidden(name, ty);
        statements.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init,
            },
            span,
        });
        hir::Expr {
            kind: hir::ExprKind::Local(local),
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }
}
