use super::*;

impl Lowerer {
    pub(super) fn inspect_enum(
        &mut self,
        context: CodingContext,
        application: hir::EnumApplicationId,
        value: hir::Expr,
        reference: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let integral = self
            .enum_definition(self.enum_applications[application].template)
            .representation
            .integer_kind()
            .is_some();
        let info = self.inspection_shape_info(
            reference,
            if integral { "IntegerEnum" } else { "Enum" },
            span,
            sink,
        )?;
        let variants = self.inspection_field(info, "variants", span)?;
        let variants = self.coding_local(variants, span, sink);
        let shape_ty = self.inspection_type("ValueShape", span)?;
        let view_ty = self.inspection_type("ValueView", span)?;
        let member_ty = self.inspection_type("MemberView", span)?;
        let result = self.alloc_hidden("inspected_variant", shape_ty);
        let mut arms = Vec::new();
        for (index, variant) in self.enum_variants(application).into_iter().enumerate() {
            self.push_scope();
            let mut body = Vec::new();
            let offset = self.inspection_long(index, span);
            let info = self.inspection_index(variants.clone(), offset, span);
            let info = self.coding_local(info, span, &mut body);
            let mut patterns = Vec::new();
            let shape = if integral {
                let view =
                    self.inspection_construct("IntegerEnumView", vec![info], span, &mut body)?;
                self.inspection_variant("ValueShape", "IntegerEnum", vec![view], span)?
            } else {
                let mut members = Vec::new();
                if !variant.fields.is_empty() {
                    let payload = self.inspection_field(info.clone(), "payload", span)?;
                    let fields = self.inspection_payload(
                        payload,
                        if variant.style == hir::VariantStyle::Positional {
                            "Positional"
                        } else {
                            "Named"
                        },
                        span,
                        &mut body,
                    )?;
                    let fields = self.coding_local(fields, span, &mut body);
                    for (field_index, (_, ty)) in variant.fields.iter().enumerate() {
                        let name = format!("__inspection_payload_{field_index}");
                        let local = self.alloc_hidden(&name, *ty);
                        self.scopes.declare(name.clone(), local);
                        patterns.push((field_index as u32, hir::Pattern::Binding { local }));
                        let reader = self.lower_synthesized_lambda(
                            false,
                            &[],
                            view_ty,
                            span,
                            |state, _| {
                                let value = state.inspection_capture(&name, span)?;
                                let mut statements = Vec::new();
                                let value =
                                    state.inspection_read(context, value, span, &mut statements)?;
                                Some(crate::stmt::ValueBlock {
                                    statements,
                                    value: Some(value),
                                })
                            },
                        )?;
                        let offset = self.inspection_long(field_index, span);
                        let field = self.inspection_index(fields.clone(), offset, span);
                        let field =
                            self.inspection_variant("ObservedMember", "Field", vec![field], span)?;
                        members.push(self.inspection_construct(
                            "MemberView",
                            vec![field, reader],
                            span,
                            &mut body,
                        )?);
                    }
                }
                let members = self.inspection_array(member_ty, members, span);
                let view =
                    self.inspection_construct("VariantView", vec![info, members], span, &mut body)?;
                self.inspection_variant("ValueShape", "Variant", vec![view], span)?
            };
            body.push(hir::Statement {
                kind: hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Binding { local: result },
                    init: shape,
                },
                span,
            });
            arms.push(hir::WhenArm {
                condition: hir::WhenCondition::Case(hir::Pattern::Variant {
                    application: variant.application,
                    fields: patterns,
                }),
                guard: None,
                body,
                span,
            });
            self.pop_scope();
        }
        sink.push(hir::Statement {
            kind: hir::StatementKind::When(hir::When::Subject {
                subject: value,
                arms,
                fallback: hir::WhenFallback::Impossible(
                    hir::ExhaustivenessProof::EnumPatternMatrix,
                ),
            }),
            span,
        });
        Some(self.coding_expr(hir::ExprKind::Local(result), shape_ty, span))
    }
}
