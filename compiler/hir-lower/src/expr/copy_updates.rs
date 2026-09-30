use std::collections::{HashMap, HashSet};

use super::*;

#[derive(Clone, Copy)]
struct PlannedField<'a> {
    source: &'a ast::FieldUpdate,
    field: hir::AppliedStructFieldRef,
    ty: TypeId,
}

struct StructCopyTarget {
    application: hir::StructApplicationId,
    fields: Vec<(hir::AppliedStructFieldRef, TypeId)>,
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
        let Type::Struct(application) = self.types[base_ty].clone() else {
            let found = self.type_name(base_ty);
            self.error(
                span,
                format!("copy update requires an exact declared struct value, found {found}"),
            );
            return None;
        };
        let (target, planned) = self.plan_struct_copy(application, fields)?;
        let base = self.materialize_copy_temporary("copy.base", base, span, sink);
        self.commit_struct_copy(base, base_ty, target, planned, span, sink)
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
        let hir::StructRepresentation::Declared(declaration_fields) = self.structs
            [self.struct_id(declaration)]
        .representation
        .clone() else {
            let found = self.type_name(application_value.canonical_type);
            self.error(
                updates.first().field.span,
                format!("copy update requires an exact declared struct value, found {found}"),
            );
            return None;
        };
        if !self.validate_copy_field_names(updates) {
            return None;
        }

        let mut fields = Vec::with_capacity(declaration_fields.len());
        for (index, field) in declaration_fields.iter().enumerate() {
            let reference = hir::AppliedStructFieldRef::checked(
                &self.structs,
                &self.struct_applications,
                self.nominal_identities
                    .as_ref()
                    .expect("nominal identities precede application references"),
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
                        self.structs[self.struct_id(declaration)].name,
                        update.field.text
                    ),
                );
                return None;
            };
            planned.push(PlannedField {
                source: update,
                field: fields[index].0,
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
                updates.get(&field).cloned().unwrap_or(hir::Expr {
                    kind: ExprKind::FieldAccess {
                        receiver: Box::new(base.clone()),
                        field: self
                            .struct_field_reference(field.application(), field.local_index()),
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

    fn lower_copy_rhs(
        &mut self,
        planned: Vec<PlannedField<'_>>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<HashMap<hir::AppliedStructFieldRef, hir::Expr>> {
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
}
