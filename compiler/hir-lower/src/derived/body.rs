//! Complete typed bodies for derived equality applications.

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, Type};

mod enums;
mod open_enums;

impl Lowerer {
    pub(super) fn build_derived_equality_body(
        &mut self,
        ty: hir::TypeId,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<hir::Body, String> {
        let mut locals = Arena::new();
        let this = self.alloc_derived_local(
            &mut locals,
            "this",
            ty,
            scoop_identity::LocalValueSelector::This,
        );
        let other = self.alloc_derived_local(
            &mut locals,
            "other",
            ty,
            scoop_identity::LocalValueSelector::Parameter {
                declaration_index: 0,
            },
        );
        let this_expr = self.local_expr(this, ty, span);
        let other_expr = self.local_expr(other, ty, span);
        let statements = match self.types[ty].clone() {
            Type::Unit => vec![return_statement(
                self.bool_expr(true, self.boolean, span),
                span,
            )],
            Type::Tuple(elements) => {
                let mut comparisons = Vec::with_capacity(elements.len());
                for (index, element) in elements.into_iter().enumerate() {
                    let field = hir::FieldRef::TupleIndex(index as u32);
                    comparisons.push(self.build_derived_field_comparison(
                        element,
                        self.field_expr(this_expr.clone(), field, element, span),
                        self.field_expr(other_expr.clone(), field, element, span),
                        &format!("{}._{}", self.type_name(ty), index + 1),
                        span,
                        stack,
                    )?);
                }
                vec![return_statement(
                    self.fold_conjunction(comparisons, self.boolean, span),
                    span,
                )]
            }
            Type::Struct(_) => {
                let structure = self
                    .struct_fields(ty)
                    .expect("a struct has complete fields");
                let mut comparisons = Vec::with_capacity(structure.fields.len());
                for field in structure.fields {
                    let left = self.field_expr(this_expr.clone(), field.reference, field.ty, span);
                    let right =
                        self.field_expr(other_expr.clone(), field.reference, field.ty, span);
                    let path = format!("{}.{}", structure.name, field.name);
                    comparisons.push(self.build_derived_field_comparison(
                        field.ty, left, right, &path, span, stack,
                    )?);
                }
                vec![return_statement(
                    self.fold_conjunction(comparisons, self.boolean, span),
                    span,
                )]
            }
            Type::Enum(_) if self.type_contains_param(ty) => vec![return_statement(
                self.build_open_enum_equality(ty, this_expr, other_expr, span, stack)?,
                span,
            )],
            Type::Enum(_) => self.build_derived_enum_equality(
                ty,
                this_expr,
                other_expr,
                &mut locals,
                span,
                stack,
            )?,
            _ => unreachable!("only Unit, tuple, struct and enum have derived equality"),
        };
        Ok(hir::Body { locals, statements })
    }

    fn build_derived_field_comparison(
        &mut self,
        ty: hir::TypeId,
        lhs: hir::Expr,
        rhs: hir::Expr,
        path: &str,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<hir::Expr, String> {
        if self.maybe_uninit_value_type(ty).is_some() {
            return self.resolve_derived_field_member_equality(lhs, rhs, path, span);
        }
        match self.types[ty].clone() {
            Type::Unit | Type::Tuple(_) => {
                let (_, application) =
                    self.ensure_structural_derived_equality_application(ty, span, stack)?;
                Ok(self.derived_call(lhs, rhs, application, self.boolean, span))
            }
            Type::Struct(_) | Type::Enum(_)
                if self.dependency_nominal_application(ty).is_some() =>
            {
                if let Some(target) = self.imported_derived_equality(ty) {
                    return Ok(self.imported_equality_call(target, lhs, rhs, span));
                }
                let (_, arguments) = self
                    .dependency_nominal_application(ty)
                    .expect("an imported equality field retains its declaration");
                if !arguments.is_empty() && !self.has_imported_same_type_equals(ty)? {
                    match self.ensure_structural_derived_equality_application(ty, span, stack) {
                        Ok((_, application)) => {
                            return Ok(self.derived_call(
                                lhs,
                                rhs,
                                application,
                                self.boolean,
                                span,
                            ));
                        }
                        Err(reason) => {
                            return self
                                .resolve_derived_field_member_equality(lhs, rhs, path, span)
                                .map_err(|_| reason);
                        }
                    }
                }
                self.resolve_derived_field_member_equality(lhs, rhs, path, span)
            }
            Type::Struct(application) => {
                let function = self.structs
                    [self.struct_id(self.struct_applications[application].template)]
                .derived_equality;
                if let Some(function) = function {
                    let owner = hir::MethodOwnerApplication::Struct(application);
                    match self.ensure_derived_equality_application(
                        ty,
                        function,
                        hir::DerivedEqualityOrigin::Nominal(owner),
                        span,
                        stack,
                    ) {
                        Ok(application) => {
                            return Ok(self.derived_call(
                                lhs,
                                rhs,
                                application,
                                self.boolean,
                                span,
                            ));
                        }
                        Err(reason) => {
                            return self
                                .resolve_derived_field_member_equality(lhs, rhs, path, span)
                                .map_err(|_| reason);
                        }
                    }
                }
                self.resolve_derived_field_member_equality(lhs, rhs, path, span)
            }
            Type::Enum(application) => {
                let function = self.enums
                    [self.enum_id(self.enum_applications[application].template)]
                .derived_equality;
                if let Some(function) = function {
                    let owner = hir::MethodOwnerApplication::Enum(application);
                    match self.ensure_derived_equality_application(
                        ty,
                        function,
                        hir::DerivedEqualityOrigin::Nominal(owner),
                        span,
                        stack,
                    ) {
                        Ok(application) => {
                            return Ok(self.derived_call(
                                lhs,
                                rhs,
                                application,
                                self.boolean,
                                span,
                            ));
                        }
                        Err(reason) => {
                            return self
                                .resolve_derived_field_member_equality(lhs, rhs, path, span)
                                .map_err(|_| reason);
                        }
                    }
                }
                self.resolve_derived_field_member_equality(lhs, rhs, path, span)
            }
            _ => self.resolve_derived_field_member_equality(lhs, rhs, path, span),
        }
    }

    fn alloc_derived_local(
        &mut self,
        locals: &mut Arena<hir::Local>,
        name: &str,
        ty: hir::TypeId,
        selector: scoop_identity::LocalValueSelector,
    ) -> hir::LocalId {
        locals.alloc(hir::Local {
            binding: self.fresh_binding(),
            selector,
            definition: hir::LocalValueDefinitionSite::Synthetic,
            name: name.to_string(),
            ty,
            mutable: false,
        })
    }
}

fn next_derived_synthetic_selector(ordinal: &mut u32) -> scoop_identity::LocalValueSelector {
    let current = *ordinal;
    *ordinal = ordinal
        .checked_add(1)
        .expect("one derived equality body cannot exhaust synthetic local ordinals");
    scoop_identity::LocalValueSelector::Synthetic {
        path: scoop_identity::StructuralDefinitionPath::from_first(
            scoop_identity::StructuralPathSegment::new(
                scoop_identity::StructuralDefinitionSiteRole::SyntheticValue,
                current,
            ),
            [],
        ),
        role: scoop_identity::SyntheticLocalRole::Temporary,
    }
}

impl Lowerer {
    fn derived_call(
        &self,
        receiver: hir::Expr,
        other: hir::Expr,
        application: hir::DerivedEqualityApplicationId,
        boolean: hir::TypeId,
        span: ast::Span,
    ) -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee: hir::MethodCallee::DerivedEquality(application),
                args: vec![other],
            },
            ty: boolean,
            span,
            origin: self.expression_origin(span),
        }
    }

    fn local_expr(&self, local: hir::LocalId, ty: hir::TypeId, span: ast::Span) -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::Local(local),
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    fn field_expr(
        &self,
        receiver: hir::Expr,
        field: hir::FieldRef,
        ty: hir::TypeId,
        span: ast::Span,
    ) -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::FieldAccess {
                receiver: Box::new(receiver),
                field,
            },
            ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    fn bool_expr(&self, value: bool, boolean: hir::TypeId, span: ast::Span) -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::BoolLiteral(value),
            ty: boolean,
            span,
            origin: self.expression_origin(span),
        }
    }

    fn fold_conjunction(
        &self,
        comparisons: Vec<hir::Expr>,
        boolean: hir::TypeId,
        span: ast::Span,
    ) -> hir::Expr {
        comparisons
            .into_iter()
            .reduce(|lhs, rhs| hir::Expr {
                kind: hir::ExprKind::Binary {
                    op: hir::BinOp::And,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                ty: boolean,
                span,
                origin: self.expression_origin(span),
            })
            .unwrap_or_else(|| self.bool_expr(true, boolean, span))
    }
}

fn return_statement(value: hir::Expr, span: ast::Span) -> hir::Statement {
    hir::Statement {
        kind: hir::StatementKind::Return { value: Some(value) },
        span,
    }
}
