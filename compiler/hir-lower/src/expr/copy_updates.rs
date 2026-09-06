use std::collections::{HashMap, HashSet};

use super::*;

#[derive(Clone, Copy)]
struct PlannedField<'a> {
    source: &'a ast::FieldUpdate,
    field: CopyFieldRef,
    ty: TypeId,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum CopyFieldRef {
    Struct(hir::AppliedStructFieldRef),
    Enum(hir::AppliedEnumVariantFieldRef),
}

struct StructCopyTarget {
    application: hir::StructApplicationId,
    fields: Vec<(hir::AppliedStructFieldRef, TypeId)>,
}

struct EnumCopyTarget {
    variant: hir::AppliedEnumVariantRef,
    fields: Vec<(hir::AppliedEnumVariantFieldRef, TypeId)>,
}

impl Lowerer {
    pub(super) fn lower_copy_update(
        &mut self,
        base: &ast::Expr,
        fields: &ast::NonEmptyVec<ast::FieldUpdate>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        match self.probe_expr_layer(|state, local_sink| {
            state.lower_copy_update_inner(base, fields, span, local_sink, expected)
        }) {
            Ok(layer) => Some(self.commit_expr_layer(layer, sink)),
            Err(failed) => {
                if failed.diagnostics.len() > self.diagnostics.len() {
                    self.commit_layer_diagnostics(*failed);
                }
                None
            }
        }
    }

    fn lower_copy_update_inner(
        &mut self,
        base: &ast::Expr,
        fields: &ast::NonEmptyVec<ast::FieldUpdate>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let base = self.lower_expr(base, sink, expected)?;
        let base_ty = base.ty;
        let base = self.materialize_copy_temporary("copy.base", base, span, sink);
        if !self.validate_copy_field_names(fields) {
            return None;
        }

        match self.types[base_ty].clone() {
            Type::Struct(application) => {
                let (target, planned) = self.plan_struct_copy(application, fields)?;
                self.commit_struct_copy(base, base_ty, target, planned, span, sink)
            }
            Type::Enum(application) => {
                let (target, planned) = self.plan_enum_copy(application, fields, span)?;
                self.commit_enum_copy(base, base_ty, target, planned, span, sink)
            }
            _ => {
                let found = self.type_name(base_ty);
                self.error(
                    span,
                    format!(
                        "copy update requires an exact declared struct or enum value, found {found}"
                    ),
                );
                None
            }
        }
    }

    fn validate_copy_field_names(&mut self, fields: &ast::NonEmptyVec<ast::FieldUpdate>) -> bool {
        let mut seen = HashSet::new();
        for field in fields.iter() {
            if !seen.insert(field.field.text.as_str()) {
                self.error(
                    field.field.span,
                    format!(
                        "copy update field `{}` is specified more than once",
                        field.field.text
                    ),
                );
                return false;
            }
        }
        true
    }

    fn plan_struct_copy<'a>(
        &mut self,
        application: hir::StructApplicationId,
        updates: &'a ast::NonEmptyVec<ast::FieldUpdate>,
    ) -> Option<(StructCopyTarget, Vec<PlannedField<'a>>)> {
        let application_value = self.struct_applications[application].clone();
        let declaration = application_value.template;
        let hir::StructRepresentation::Declared(declaration_fields) =
            self.structs[declaration].representation.clone()
        else {
            let found = self.type_name(application_value.canonical_type);
            self.error(
                updates.first().field.span,
                format!(
                    "copy update requires an exact declared struct or enum value, found {found}"
                ),
            );
            return None;
        };

        let mut fields = Vec::with_capacity(declaration_fields.len());
        for (index, field) in declaration_fields.iter().enumerate() {
            let reference = hir::AppliedStructFieldRef::checked(
                &self.structs,
                &self.struct_applications,
                application,
                index as u32,
            )
            .expect("a declared struct field produces a checked applied reference");
            let ty = self.instantiate_ty(field.ty, &application_value.arguments);
            fields.push((reference, ty));
        }

        let mut planned = Vec::with_capacity(updates.len());
        for update in updates.iter() {
            let Some((index, (_, ty))) = fields
                .iter()
                .enumerate()
                .find(|(index, _)| declaration_fields[*index].name == update.field.text)
            else {
                self.error(
                    update.field.span,
                    format!(
                        "struct `{}` has no field `{}`",
                        self.structs[declaration].name, update.field.text
                    ),
                );
                return None;
            };
            planned.push(PlannedField {
                source: update,
                field: CopyFieldRef::Struct(fields[index].0),
                ty: *ty,
            });
        }
        Some((
            StructCopyTarget {
                application,
                fields,
            },
            planned,
        ))
    }

    fn plan_enum_copy<'a>(
        &mut self,
        application: hir::EnumApplicationId,
        updates: &'a ast::NonEmptyVec<ast::FieldUpdate>,
        span: Span,
    ) -> Option<(EnumCopyTarget, Vec<PlannedField<'a>>)> {
        let application_value = self.enum_applications[application].clone();
        let enumeration = application_value.template;
        let candidates: Vec<u32> = self.enums[enumeration]
            .variants
            .iter()
            .enumerate()
            .filter(|(index, variant)| {
                matches!(
                    self.variant_styles.get(&(enumeration, *index as u32)),
                    Some(VariantStyle::Named | VariantStyle::Constructor)
                ) && updates.iter().all(|update| {
                    variant
                        .fields
                        .iter()
                        .any(|field| field.name == update.field.text)
                })
            })
            .map(|(index, _)| index as u32)
            .collect();
        let variant_index = match candidates.as_slice() {
            [candidate] => *candidate,
            [] => {
                let fields = updates
                    .iter()
                    .map(|update| format!("`{}`", update.field.text))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.error(
                    span,
                    format!(
                        "enum `{}` has no named-payload variant containing all copy-update fields: {fields}",
                        self.enums[enumeration].name
                    ),
                );
                return None;
            }
            _ => {
                let variants = candidates
                    .iter()
                    .map(|index| {
                        format!(
                            "`{}`",
                            self.enums[enumeration].variants[*index as usize].name
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                self.error(
                    span,
                    format!(
                        "copy update of enum `{}` is ambiguous between variants {variants}; use `when` to rebuild the intended variant explicitly",
                        self.enums[enumeration].name
                    ),
                );
                return None;
            }
        };

        let variant = hir::AppliedEnumVariantRef::checked_index(
            &self.enums,
            &self.enum_applications,
            application,
            variant_index,
        )
        .expect("an enum candidate produces a checked applied variant reference");
        let declaration_fields = self.enums[enumeration].variants[variant_index as usize]
            .fields
            .clone();
        let mut fields = Vec::with_capacity(declaration_fields.len());
        for (index, field) in declaration_fields.iter().enumerate() {
            let reference =
                hir::AppliedEnumVariantFieldRef::checked(&self.enums, variant, index as u32)
                    .expect("a variant field produces a checked applied reference");
            let ty = self.instantiate_ty(field.ty, &application_value.arguments);
            fields.push((reference, ty));
        }

        let mut planned = Vec::with_capacity(updates.len());
        for update in updates.iter() {
            let (index, (_, ty)) = fields
                .iter()
                .enumerate()
                .find(|(index, _)| declaration_fields[*index].name == update.field.text)
                .expect("the chosen enum candidate contains every update field");
            planned.push(PlannedField {
                source: update,
                field: CopyFieldRef::Enum(fields[index].0),
                ty: *ty,
            });
        }
        Some((EnumCopyTarget { variant, fields }, planned))
    }

    fn commit_struct_copy(
        &mut self,
        base: hir::Expr,
        result_ty: TypeId,
        target: StructCopyTarget,
        planned: Vec<PlannedField<'_>>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let updates = self.lower_copy_rhs(planned, sink)?;
        let fields = target
            .fields
            .into_iter()
            .map(|(field, ty)| {
                updates
                    .get(&CopyFieldRef::Struct(field))
                    .cloned()
                    .unwrap_or(hir::Expr {
                        kind: ExprKind::FieldAccess {
                            receiver: Box::new(base.clone()),
                            field: hir::FieldRef::StructField(field),
                        },
                        ty,
                        span,
                        origin: self.expression_origin(span),
                    })
            })
            .collect();
        Some(hir::Expr {
            kind: ExprKind::StructConstruct {
                application: target.application,
                fields,
            },
            ty: result_ty,
            span,
            origin: self.expression_origin(span),
        })
    }

    fn commit_enum_copy(
        &mut self,
        base: hir::Expr,
        result_ty: TypeId,
        target: EnumCopyTarget,
        planned: Vec<PlannedField<'_>>,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let origin = self.expression_origin(span);
        let condition = hir::Expr {
            kind: ExprKind::VariantTest {
                operand: Box::new(base.clone()),
                variant: target.variant,
            },
            ty: self.boolean,
            span,
            origin,
        };
        let mut then_body = Vec::new();
        let preserved_fields = target
            .fields
            .iter()
            .map(|(field, ty)| {
                let projected = hir::Expr {
                    kind: ExprKind::VariantPayloadProject {
                        operand: Box::new(base.clone()),
                        field: *field,
                    },
                    ty: *ty,
                    span,
                    origin,
                };
                self.materialize_copy_temporary("copy.payload", projected, span, &mut then_body)
            })
            .collect::<Vec<_>>();
        let updates = self.lower_copy_rhs(planned, &mut then_body)?;
        let arguments = target
            .fields
            .into_iter()
            .enumerate()
            .map(|(index, (field, _))| {
                updates
                    .get(&CopyFieldRef::Enum(field))
                    .cloned()
                    .unwrap_or_else(|| preserved_fields[index].clone())
            })
            .collect();
        let result = self.alloc_hidden_result(result_ty);
        then_body.push(hir::Statement {
            kind: hir::StatementKind::Assign {
                target: hir::AssignTarget::Local(result),
                value: hir::Expr {
                    kind: ExprKind::VariantConstruct {
                        variant: target.variant,
                        args: arguments,
                    },
                    ty: result_ty,
                    span,
                    origin,
                },
            },
            span,
        });
        let exception = self.copy_update_variant_mismatch(target.variant, span)?;
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond: condition,
                then_body,
                else_body: Some(vec![hir::Statement {
                    kind: hir::StatementKind::Throw(exception),
                    span,
                }]),
            },
            span,
        });
        Some(hir::Expr {
            kind: ExprKind::Local(result),
            ty: result_ty,
            span,
            origin,
        })
    }

    fn lower_copy_rhs(
        &mut self,
        planned: Vec<PlannedField<'_>>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<HashMap<CopyFieldRef, hir::Expr>> {
        let mut lowered = HashMap::with_capacity(planned.len());
        for field in planned {
            let value = self.lower_expr(&field.source.value, sink, Some(field.ty))?;
            if !self.is_subtype(value.ty, field.ty) {
                let expected = self.type_name(field.ty);
                let found = self.type_name(value.ty);
                self.error(
                    field.source.value.span(),
                    format!(
                        "copy update field `{}` expects {expected}, found {found}",
                        field.source.field.text
                    ),
                );
                return None;
            }
            let value = self.adapt_to(value, field.ty);
            let value =
                self.materialize_copy_temporary("copy.update", value, field.source.span, sink);
            assert!(
                lowered.insert(field.field, value).is_none(),
                "duplicate update fields are rejected before RHS lowering"
            );
        }
        Some(lowered)
    }

    fn materialize_copy_temporary(
        &mut self,
        prefix: &str,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let ty = value.ty;
        let local = self.alloc_hidden(prefix, ty);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init: value,
            },
            span,
        });
        hir::Expr {
            kind: ExprKind::Local(local),
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    fn copy_update_variant_mismatch(
        &mut self,
        variant: hir::AppliedEnumVariantRef,
        span: Span,
    ) -> Option<hir::Expr> {
        let target = self.illegal_state_message_constructor?;
        let enum_name =
            self.type_name(self.enum_applications[variant.application()].canonical_type);
        let declaration = variant.declaration();
        let variant_name = &self.enums[declaration.enumeration()].variants
            [declaration.local_index() as usize]
            .name;
        let message = format!("copy update expected enum `{enum_name}` variant `{variant_name}`");
        let option_string = self.option_type(self.string);
        let message = hir::Expr {
            kind: ExprKind::SomeWrap(Box::new(hir::Expr {
                kind: ExprKind::StringLiteral(message),
                ty: self.string,
                span,
                origin: self.expression_origin(span),
            })),
            ty: option_string,
            span,
            origin: self.expression_origin(span),
        };
        let owner = self.classes[target.class].self_application;
        let constructor = self.class_constructor_application(target.constructor, owner);
        Some(hir::Expr {
            kind: ExprKind::ClassInit {
                constructor,
                args: vec![message],
            },
            ty: self.class_applications[owner].canonical_type,
            span,
            origin: self.expression_origin(span),
        })
    }
}
