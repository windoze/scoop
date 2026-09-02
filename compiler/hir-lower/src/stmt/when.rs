use super::*;

struct ValueArm {
    pattern: hir::Pattern,
    guard: Option<hir::Expr>,
    body: ValueBlock,
    span: Span,
}

impl Lowerer {
    /// Statement-position pattern `when` (spec 5). The subject must be an
    /// enum, tuple or struct — the current pattern-matching subset has no
    /// Kotlin-style condition `when`. Pattern bindings
    /// scope over the arm's guard and body; exhaustiveness is checked
    /// over the whole statement.
    pub(super) fn lower_when(
        &mut self,
        when: &ast::When,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let subject = self.lower_expr(&when.subject, &mut sink, None)?;
        if !matches!(
            self.types[subject.ty],
            Type::Enum(..) | Type::Tuple(..) | Type::Struct(..)
        ) {
            let found = self.type_name(subject.ty);
            self.error(
                when.subject.span(),
                format!("`when` subject must be an enum, tuple or struct, found {found}"),
            );
            return None;
        }
        // The subject is evaluated exactly once, right before the
        // `when`, so desugaring statements belong before it.
        out.extend(sink);

        let mut arms = Vec::with_capacity(when.arms.len());
        let mut arms_ok = true;
        for arm in &when.arms {
            self.push_scope();
            let lowered = self.lower_arm(arm, subject.ty);
            self.pop_scope();
            match lowered {
                Some(arm) => arms.push(arm),
                None => arms_ok = false,
            }
        }
        let else_body = when.else_body.as_ref().map(|b| self.lower_block(b));
        // With a failed arm the coverage information is unreliable;
        // the module is rejected anyway, so skip the exhaustiveness
        // check to avoid noise.
        if arms_ok {
            self.check_exhaustiveness(when.span, subject.ty, &arms, else_body.is_some());
        }
        Some(hir::StatementKind::When(hir::When {
            subject,
            arms,
            else_body,
        }))
    }

    /// Pattern `when` in value position. Arms that need context are lowered
    /// after context-independent arms establish a provisional result type;
    /// source order is restored in the resulting HIR.
    pub(crate) fn lower_when_expression(
        &mut self,
        when: &ast::When,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let mut subject_sink = Vec::new();
        let subject = self.lower_expr(&when.subject, &mut subject_sink, None)?;
        if !matches!(
            self.types[subject.ty],
            Type::Enum(..) | Type::Tuple(..) | Type::Struct(..)
        ) {
            let found = self.type_name(subject.ty);
            self.error(
                when.subject.span(),
                format!("`when` subject must be an enum, tuple or struct, found {found}"),
            );
            return None;
        }

        let deferred: Vec<_> = when
            .arms
            .iter()
            .map(|arm| expected.is_none() && self.value_block_requires_expected(&arm.body))
            .collect();
        let defer_else = when
            .else_body
            .as_ref()
            .is_some_and(|body| expected.is_none() && self.value_block_requires_expected(body));
        let mut arms: Vec<Option<ValueArm>> = (0..when.arms.len()).map(|_| None).collect();
        for (index, arm) in when.arms.iter().enumerate() {
            if !deferred[index] {
                arms[index] = Some(self.lower_value_arm(arm, subject.ty, expected)?);
            }
        }
        let mut else_value = match &when.else_body {
            Some(body) if !defer_else => Some(self.lower_value_block(body, expected)?),
            _ => None,
        };
        let mut hint_types: Vec<_> = arms
            .iter()
            .filter_map(|arm| {
                arm.as_ref()
                    .and_then(|arm| arm.body.value.as_ref().map(|value| value.ty))
            })
            .collect();
        if let Some(ty) = else_value
            .as_ref()
            .and_then(|body| body.value.as_ref().map(|value| value.ty))
        {
            hint_types.push(ty);
        }
        let hint = (!hint_types.is_empty()).then(|| self.least_upper_bound(&hint_types));
        for (index, arm) in when.arms.iter().enumerate() {
            if arms[index].is_none() {
                arms[index] = Some(self.lower_value_arm(arm, subject.ty, hint)?);
            }
        }
        if defer_else {
            else_value =
                Some(self.lower_value_block(
                    when.else_body.as_ref().expect("deferred else exists"),
                    hint,
                )?);
        }
        let mut arms: Vec<ValueArm> = arms
            .into_iter()
            .map(|arm| arm.expect("every when arm was lowered"))
            .collect();
        let mut block_refs: Vec<&mut ValueBlock> =
            arms.iter_mut().map(|arm| &mut arm.body).collect();
        if let Some(else_value) = else_value.as_mut() {
            block_refs.push(else_value);
        }
        let result =
            self.finish_control_value("when", when.span, expected, block_refs.as_mut_slice())?;
        drop(block_refs);
        let arms: Vec<_> = arms
            .into_iter()
            .map(|arm| hir::WhenArm {
                pattern: arm.pattern,
                guard: arm.guard,
                body: arm.body.statements,
                span: arm.span,
            })
            .collect();
        self.check_exhaustiveness(when.span, subject.ty, &arms, else_value.is_some());
        sink.extend(subject_sink);
        sink.push(hir::Statement {
            span: when.span,
            kind: hir::StatementKind::When(hir::When {
                subject,
                arms,
                else_body: else_value.map(|body| body.statements),
            }),
        });
        Some(result)
    }

    /// One `when` arm: pattern (its bindings are in scope), optional
    /// guard (must be `Boolean`), body block.
    fn lower_arm(&mut self, arm: &ast::WhenArm, subject_ty: TypeId) -> Option<hir::WhenArm> {
        let (pattern, guard) = self.lower_arm_head(arm, subject_ty)?;
        let body = self.lower_block(&arm.body);
        Some(hir::WhenArm {
            pattern,
            guard,
            body,
            span: arm.span,
        })
    }

    fn lower_value_arm(
        &mut self,
        arm: &ast::WhenArm,
        subject_ty: TypeId,
        expected: Option<TypeId>,
    ) -> Option<ValueArm> {
        self.push_scope();
        let lowered = (|| {
            let (pattern, guard) = self.lower_arm_head(arm, subject_ty)?;
            let body = self.lower_value_block(&arm.body, expected)?;
            Some(ValueArm {
                pattern,
                guard,
                body,
                span: arm.span,
            })
        })();
        self.pop_scope();
        lowered
    }

    fn lower_arm_head(
        &mut self,
        arm: &ast::WhenArm,
        subject_ty: TypeId,
    ) -> Option<(hir::Pattern, Option<hir::Expr>)> {
        let pattern = self.lower_pattern(
            &arm.pattern,
            subject_ty,
            PatternCtx {
                mutable: false,
                in_when: true,
            },
        )?;
        let guard = match &arm.guard {
            Some(guard) => {
                let mut sink = Vec::new();
                let guard_expr = self.lower_expr(guard, &mut sink, None)?;
                // A guard is evaluated once per arm attempt, but sink
                // statements would execute unconditionally before the
                // arm body; reject desugaring operators (same rule as
                // while conditions).
                if !sink.is_empty() {
                    self.error(
                        guard.span(),
                        "`?.` and `?:` are not allowed in a when guard".to_string(),
                    );
                    return None;
                }
                if guard_expr.ty != self.boolean {
                    let found = self.type_name(guard_expr.ty);
                    self.error(
                        guard.span(),
                        format!("when guard must be Boolean, found {found}"),
                    );
                    return None;
                }
                Some(guard_expr)
            }
            None => None,
        };
        Some((pattern, guard))
    }
}
