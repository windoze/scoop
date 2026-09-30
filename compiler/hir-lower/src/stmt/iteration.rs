use super::*;
use crate::expr::{CallSite, RequiredCallableModifiers};

impl Lowerer {
    pub(super) fn lower_for(&mut self, source: &ast::For) -> Option<Vec<hir::Statement>> {
        self.with_pattern_transaction(|state| state.lower_for_inner(source))
    }

    fn lower_for_inner(&mut self, source: &ast::For) -> Option<Vec<hir::Statement>> {
        let mut statements = Vec::new();
        let source_init = self.lower_expr(&source.iterable, &mut statements, None)?;
        let source_receiver = self.save_iteration_value(
            "for.source",
            source_init,
            source.iterable.span(),
            &mut statements,
        );
        let iterator_call = self.lower_named_call_on_receiver(
            source_receiver,
            &ast::Ident {
                text: "iterator".to_string(),
                span: source.iterable.span(),
            },
            CallSite {
                type_args: &[],
                args: &[],
                span: source.iterable.span(),
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
                    source.iterable.span(),
                    format!(
                        "iterator operator result type `{found}` does not implement `Iterator<T>`"
                    ),
                );
                return None;
            }
            _ => {
                let found = self.type_name(iterator_call.ty);
                self.error(
                    source.iterable.span(),
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
        let raw_iterator = self.save_iteration_value(
            "for.iterator.result",
            iterator_call,
            source.span,
            &mut statements,
        );
        let adapted = self.adapt_to(raw_iterator, iterator_type);
        let iterator =
            self.save_iteration_value("for.iterator", adapted, source.span, &mut statements);
        let option_type = self.option_type(element_type);
        let next_call = self.lower_iteration_next(iterator, option_type)?;
        let mut condition_setup = Vec::new();
        let next_result =
            self.save_iteration_value("for.next", next_call, source.span, &mut condition_setup);
        let origin = self.expression_origin(source.span);
        let cond = hir::Expr {
            kind: hir::ExprKind::IsSome(Box::new(next_result.clone())),
            ty: self.boolean,
            span: source.span,
            origin,
        };
        let element_init = hir::Expr {
            kind: hir::ExprKind::Unwrap {
                operand: Box::new(next_result),
                trap_on_none: false,
            },
            ty: element_type,
            span: source.span,
            origin,
        };
        let mut body = Vec::new();
        let element =
            self.save_iteration_value("for.element", element_init, source.span, &mut body);
        let hir::ExprKind::Local(local) = element.kind else {
            unreachable!("a saved iteration value is a local")
        };
        let target = self.fresh_loop();
        self.push_scope();
        let planned = (|| {
            let binding = self.lower_irrefutable_binding_from_subject(
                &source.pattern,
                crate::patterns::BindingSubject {
                    local,
                    ty: element_type,
                },
                false,
            )?;
            body.extend(binding);
            self.loop_targets.push(target);
            for statement in &source.body.statements {
                self.lower_statement(statement, &mut body);
            }
            assert_eq!(self.loop_targets.pop(), Some(target));
            Some(())
        })();
        self.pop_scope();
        planned?;

        statements.push(hir::Statement {
            kind: hir::StatementKind::While {
                target,
                condition_setup,
                cond,
                body,
            },
            span: source.span,
        });
        Some(statements)
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
