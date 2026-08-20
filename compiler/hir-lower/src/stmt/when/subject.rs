use super::*;

impl Lowerer {
    pub(super) fn lower_when_subject(
        &mut self,
        when: &ast::When,
        out: &mut Vec<hir::Statement>,
    ) -> Option<Option<Subject>> {
        let incoming = self.smart_casts.clone();
        let start = out.len();
        let result = self.lower_when_subject_inner(when, out);
        self.advance_smart_casts(incoming, &out[start..], None);
        result
    }

    fn lower_when_subject_inner(
        &mut self,
        when: &ast::When,
        out: &mut Vec<hir::Statement>,
    ) -> Option<Option<Subject>> {
        let Some(initializer) = when.subject.initializer() else {
            return Some(None);
        };
        let declaration = match &when.subject {
            ast::WhenSubject::Declaration(declaration) => Some(declaration.as_ref()),
            _ => None,
        };
        let expected = match declaration.and_then(|declaration| declaration.ty.as_ref()) {
            Some(ty) => Some(self.resolve_type_ref(ty)?),
            None => None,
        };
        let mut value = if expected.is_none() {
            self.lower_type_check_operand(initializer, out)?
        } else {
            self.lower_expr(initializer, out, expected)?
        };
        if let Some(expected) = expected {
            if !self.is_subtype(value.ty, expected) {
                let found = self.type_name(value.ty);
                let expected_name = self.type_name(expected);
                self.error(
                    initializer.span(),
                    format!(
                        "when subject initializer must be of type {expected_name}, found {found}"
                    ),
                );
                return None;
            }
            value = self.adapt_to(value, expected);
        }
        // Pattern-only subjects already have an exactly-once evaluation point
        // in MIR. Ordinary conditions also need a front-end local to reference.
        if declaration.is_none()
            && !when
                .arms
                .iter()
                .any(|arm| matches!(arm.condition, ast::WhenArmCondition::Conditions(_)))
        {
            return Some(Some(Subject {
                value,
                reference: initializer.clone(),
            }));
        }
        let span = initializer.span();
        let local = self.alloc_hidden("when.subject", value.ty);
        let name = ast::Ident {
            text: self.locals[local].name.clone(),
            span,
        };
        self.scopes.declare(name.text.clone(), local);
        let snapshot = hir::Expr {
            kind: hir::ExprKind::Local(local),
            ty: value.ty,
            span,
            origin: self.expression_origin(span),
        };
        out.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: value,
            },
            span,
        });
        let mut reference = ast::Expr::Var(name);
        if let Some(declaration) = declaration {
            if matches!(declaration.target, ast::Pattern::Binding(_)) {
                let pattern = self.lower_pattern(
                    &declaration.target,
                    snapshot.ty,
                    PatternCtx {
                        mutable: false,
                        in_when: false,
                    },
                )?;
                out.push(hir::Statement {
                    kind: hir::StatementKind::ValDecl {
                        pattern,
                        init: snapshot.clone(),
                    },
                    span: declaration.span,
                });
                if let ast::Pattern::Binding(name) = &declaration.target {
                    reference = ast::Expr::Var(name.clone());
                }
            } else {
                out.extend(self.lower_irrefutable_binding(
                    &declaration.target,
                    snapshot.clone(),
                    false,
                    declaration.span,
                )?);
            }
        } else if let ast::Expr::Var(name) = initializer
            && self.stable_smart_cast_binding(&name.text).is_some()
            && self
                .scopes
                .lookup(&name.text)
                .is_none_or(|local| !self.locals[local].mutable)
        {
            // Only an immutable source can replace the saved subject in later
            // predicates. Mutable sources share facts until their next write.
            reference = initializer.clone();
        }
        Some(Some(Subject {
            value: snapshot,
            reference,
        }))
    }
}
