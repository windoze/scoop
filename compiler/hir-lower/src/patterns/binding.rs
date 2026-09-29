//! Irrefutable bindings lower directly to ordinary typed statements.

use super::*;

mod aggregates;

#[derive(Clone, Copy)]
pub(crate) struct BindingSubject {
    pub(crate) local: hir::LocalId,
    pub(crate) ty: TypeId,
}

impl BindingSubject {
    fn expression(self, span: Span, origin: hir::ExpressionOrigin) -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::Local(self.local),
            ty: self.ty,
            span,
            origin,
        }
    }
}

impl Lowerer {
    pub(crate) fn lower_irrefutable_binding(
        &mut self,
        pattern: &ast::Pattern,
        init: hir::Expr,
        mutable: bool,
        span: Span,
    ) -> Option<Vec<hir::Statement>> {
        self.with_pattern_transaction(|state| {
            let subject = BindingSubject {
                local: state.alloc_hidden("binding.subject", init.ty),
                ty: init.ty,
            };
            let mut statements = vec![binding_statement(subject.local, init, span)];
            state.lower_binding_pattern(pattern, subject, mutable, &mut statements)?;
            Some(statements)
        })
    }

    /// Lambda and for owners already hold the transaction and subject value.
    pub(crate) fn lower_irrefutable_binding_from_subject(
        &mut self,
        pattern: &ast::Pattern,
        subject: BindingSubject,
        mutable: bool,
    ) -> Option<Vec<hir::Statement>> {
        let mut statements = Vec::new();
        self.lower_binding_pattern(pattern, subject, mutable, &mut statements)?;
        Some(statements)
    }

    fn lower_binding_pattern(
        &mut self,
        pattern: &ast::Pattern,
        subject: BindingSubject,
        mutable: bool,
        statements: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        match pattern {
            ast::Pattern::Binding(name) => {
                let local = self.bind_local(name, subject.ty, mutable)?;
                let init = subject.expression(name.span, self.expression_origin(name.span));
                statements.push(binding_statement(local, init, name.span));
                Some(())
            }
            ast::Pattern::Wildcard { .. } => Some(()),
            ast::Pattern::Literal { span, .. } => {
                self.error(
                    *span,
                    "refutable patterns are only allowed in `when`".into(),
                );
                None
            }
            ast::Pattern::Tuple {
                elements,
                rest,
                span,
            } => match self.types[subject.ty].clone() {
                Type::Tuple(types) => {
                    let owner = format!("tuple of type {}", self.type_name(subject.ty));
                    let indices = self.positional_pattern_indices(
                        elements,
                        *rest,
                        types.len(),
                        &owner,
                        *span,
                    )?;
                    for (element, index) in elements.iter().zip(indices) {
                        self.lower_projected_binding(
                            element,
                            subject,
                            (hir::FieldRef::TupleIndex(index as u32), types[index]),
                            pattern_span(element),
                            mutable,
                            statements,
                        )?;
                    }
                    Some(())
                }
                Type::Struct(_) | Type::ImportedStruct(_) => {
                    self.lower_struct_binding(elements, *rest, *span, subject, mutable, statements)
                }
                Type::Class(_) | Type::ImportedClass(_) => {
                    self.lower_component_binding(elements, *rest, subject, mutable, statements)
                }
                _ => {
                    let found = self.type_name(subject.ty);
                    self.error(
                        *span,
                        format!("tuple pattern does not match a subject of type {found}"),
                    );
                    None
                }
            },
            ast::Pattern::Positional {
                path,
                elements,
                rest,
                span,
            } => {
                if !matches!(
                    self.resolve_pattern_path(path, subject.ty, *span)?,
                    PatternTarget::Struct(_)
                ) {
                    self.error(
                        *span,
                        "refutable patterns are only allowed in `when`".into(),
                    );
                    return None;
                }
                self.lower_struct_binding(elements, *rest, *span, subject, mutable, statements)
            }
            ast::Pattern::Named {
                path,
                fields,
                rest,
                span,
            } => {
                if !matches!(
                    self.resolve_pattern_path(path, subject.ty, *span)?,
                    PatternTarget::Struct(_)
                ) {
                    self.error(
                        *span,
                        "refutable patterns are only allowed in `when`".into(),
                    );
                    return None;
                }
                self.lower_named_struct_binding(fields, *rest, *span, subject, mutable, statements)
            }
        }
    }

    fn lower_projected_binding(
        &mut self,
        pattern: &ast::Pattern,
        subject: BindingSubject,
        projection: (hir::FieldRef, TypeId),
        span: Span,
        mutable: bool,
        statements: &mut Vec<hir::Statement>,
    ) -> Option<()> {
        if matches!(pattern, ast::Pattern::Wildcard { .. }) {
            return Some(());
        }
        let (field, ty) = projection;
        let origin = self.expression_origin(span);
        let init = hir::Expr {
            kind: hir::ExprKind::FieldAccess {
                receiver: Box::new(subject.expression(span, origin)),
                field,
            },
            ty,
            span,
            origin,
        };
        let local = self.alloc_hidden("binding.projection", ty);
        statements.push(binding_statement(local, init, span));
        self.lower_binding_pattern(pattern, BindingSubject { local, ty }, mutable, statements)
    }
}

fn binding_statement(local: hir::LocalId, init: hir::Expr, span: Span) -> hir::Statement {
    hir::Statement {
        kind: hir::StatementKind::ValDecl {
            pattern: hir::Pattern::Binding { local },
            init,
        },
        span,
    }
}
