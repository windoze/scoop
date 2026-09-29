use super::*;

impl Lowerer {
    pub(super) fn build_derived_enum_equality(
        &mut self,
        ty: hir::TypeId,
        this_expr: hir::Expr,
        other_expr: hir::Expr,
        locals: &mut Arena<hir::Local>,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<Vec<hir::Statement>, String> {
        let variants = self
            .enum_variants(ty)
            .into_iter()
            .map(|variant| {
                (
                    variant.application,
                    format!("{}.{}", variant.owner_name, variant.name),
                    variant.fields,
                )
            })
            .collect::<Vec<_>>();
        let proof = hir::ExhaustivenessProof::EnumPatternMatrix { subject_ty: ty };
        let mut synthetic_ordinal = 0;
        let mut arms = Vec::with_capacity(variants.len());
        for (variant_index, (target, path, fields)) in variants.into_iter().enumerate() {
            let mut left_fields = Vec::with_capacity(fields.len());
            let mut right_fields = Vec::with_capacity(fields.len());
            let mut comparisons = Vec::with_capacity(fields.len());
            for (field_index, (name, field_ty)) in fields.into_iter().enumerate() {
                let left = self.alloc_derived_local(
                    locals,
                    &format!("$left.{variant_index}.{field_index}"),
                    field_ty,
                    next_derived_synthetic_selector(&mut synthetic_ordinal),
                );
                let right = self.alloc_derived_local(
                    locals,
                    &format!("$right.{variant_index}.{field_index}"),
                    field_ty,
                    next_derived_synthetic_selector(&mut synthetic_ordinal),
                );
                left_fields.push((field_index as u32, hir::Pattern::Binding { local: left }));
                right_fields.push((field_index as u32, hir::Pattern::Binding { local: right }));
                let name = if name.is_empty() {
                    format!("_{}", field_index + 1)
                } else {
                    name
                };
                comparisons.push(self.build_derived_field_comparison(
                    field_ty,
                    self.local_expr(left, field_ty, span),
                    self.local_expr(right, field_ty, span),
                    &format!("{path}.{name}"),
                    span,
                    stack,
                )?);
            }
            let equal = self.fold_conjunction(comparisons, self.boolean, span);
            let inner = hir::Statement {
                kind: hir::StatementKind::When(hir::When {
                    subject: other_expr.clone(),
                    arms: vec![hir::WhenArm {
                        pattern: hir::Pattern::Variant {
                            application: target,
                            fields: right_fields,
                        },
                        guard: None,
                        body: vec![return_statement(equal, span)],
                        span,
                    }],
                    fallback: hir::WhenFallback::Else(vec![return_statement(
                        self.bool_expr(false, self.boolean, span),
                        span,
                    )]),
                }),
                span,
            };
            arms.push(hir::WhenArm {
                pattern: hir::Pattern::Variant {
                    application: target,
                    fields: left_fields,
                },
                guard: None,
                body: vec![inner],
                span,
            });
        }
        Ok(vec![hir::Statement {
            kind: hir::StatementKind::When(hir::When {
                subject: this_expr,
                arms,
                fallback: hir::WhenFallback::Impossible(proof),
            }),
            span,
        }])
    }
}
