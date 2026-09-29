use super::*;

enum VariantTarget {
    Local {
        application: hir::EnumApplicationId,
        index: u32,
    },
    Imported {
        owner: hir::TypeId,
        variant: scoop_identity::PersistentEnumVariantId,
    },
}

impl VariantTarget {
    fn pattern(&self, fields: Vec<(u32, hir::Pattern)>) -> hir::Pattern {
        match *self {
            Self::Local { application, index } => hir::Pattern::Variant {
                application,
                variant: index,
                fields,
            },
            Self::Imported { owner, variant } => hir::Pattern::ImportedVariant {
                owner,
                variant,
                fields,
            },
        }
    }
}

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
        let (variants, proof) = match self.types[ty].clone() {
            Type::Enum(application) => {
                let value = self.enum_applications[application].clone();
                let declaration = self.enums[value.template].clone();
                let variants = declaration
                    .variants
                    .into_iter()
                    .enumerate()
                    .map(|(index, variant)| {
                        let fields = variant
                            .fields
                            .into_iter()
                            .map(|field| {
                                (field.name, self.instantiate_ty(field.ty, &value.arguments))
                            })
                            .collect::<Vec<_>>();
                        (
                            VariantTarget::Local {
                                application,
                                index: index as u32,
                            },
                            format!("{}.{}", declaration.name, variant.name),
                            fields,
                        )
                    })
                    .collect::<Vec<_>>();
                (
                    variants,
                    hir::ExhaustivenessProof::EnumPatternMatrix {
                        subject_ty: ty,
                        application,
                    },
                )
            }
            Type::ImportedEnum(enumeration) => {
                let name = self.type_name(ty);
                let variants = enumeration
                    .variants
                    .iter()
                    .map(|variant| {
                        (
                            VariantTarget::Imported {
                                owner: ty,
                                variant: variant.identity,
                            },
                            format!("{name}.{}", variant.name),
                            variant
                                .fields
                                .iter()
                                .map(|field| (field.name.clone(), field.ty))
                                .collect(),
                        )
                    })
                    .collect();
                (
                    variants,
                    hir::ExhaustivenessProof::PatternMatrix { subject_ty: ty },
                )
            }
            _ => unreachable!("enum equality retains its exact enum owner"),
        };
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
                        pattern: target.pattern(right_fields),
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
                pattern: target.pattern(left_fields),
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
