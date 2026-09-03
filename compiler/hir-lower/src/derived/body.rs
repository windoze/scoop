//! Complete typed bodies for derived equality applications.

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, Type};

impl Lowerer {
    pub(super) fn build_derived_equality_body(
        &mut self,
        ty: hir::TypeId,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<hir::Body, String> {
        let mut locals = Arena::new();
        let this = self.alloc_derived_local(&mut locals, "this", ty);
        let other = self.alloc_derived_local(&mut locals, "other", ty);
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
            Type::Struct(application) => {
                let value = self.struct_applications[application].clone();
                let fields = self.structs[value.template].semantic_fields().to_vec();
                let mut comparisons = Vec::with_capacity(fields.len());
                for (index, field) in fields.into_iter().enumerate() {
                    let field_ty = self.instantiate_ty(field.ty, &value.arguments);
                    let field_ref = hir::FieldRef::StructField {
                        application,
                        index: index as u32,
                    };
                    let left = self.field_expr(this_expr.clone(), field_ref, field_ty, span);
                    let right = self.field_expr(other_expr.clone(), field_ref, field_ty, span);
                    let path = format!("{}.{}", self.structs[value.template].name, field.name);
                    comparisons.push(self.build_derived_field_comparison(
                        field_ty, left, right, &path, span, stack,
                    )?);
                }
                vec![return_statement(
                    self.fold_conjunction(comparisons, self.boolean, span),
                    span,
                )]
            }
            Type::Enum(application) => {
                let value = self.enum_applications[application].clone();
                let declaration = self.enums[value.template].clone();
                let mut arms = Vec::with_capacity(declaration.variants.len());
                for (variant_index, variant) in declaration.variants.into_iter().enumerate() {
                    let mut left_fields = Vec::with_capacity(variant.fields.len());
                    let mut right_fields = Vec::with_capacity(variant.fields.len());
                    let mut comparisons = Vec::with_capacity(variant.fields.len());
                    for (field_index, field) in variant.fields.into_iter().enumerate() {
                        let field_ty = self.instantiate_ty(field.ty, &value.arguments);
                        let left = self.alloc_derived_local(
                            &mut locals,
                            &format!("$left.{variant_index}.{field_index}"),
                            field_ty,
                        );
                        let right = self.alloc_derived_local(
                            &mut locals,
                            &format!("$right.{variant_index}.{field_index}"),
                            field_ty,
                        );
                        left_fields
                            .push((field_index as u32, hir::Pattern::Binding { local: left }));
                        right_fields
                            .push((field_index as u32, hir::Pattern::Binding { local: right }));
                        let field_name = if field.name.is_empty() {
                            format!("_{}", field_index + 1)
                        } else {
                            field.name
                        };
                        let path = format!("{}.{}.{}", declaration.name, variant.name, field_name);
                        comparisons.push(self.build_derived_field_comparison(
                            field_ty,
                            self.local_expr(left, field_ty, span),
                            self.local_expr(right, field_ty, span),
                            &path,
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
                                    application,
                                    variant: variant_index as u32,
                                    fields: right_fields,
                                },
                                guard: None,
                                body: vec![return_statement(equal, span)],
                                span,
                            }],
                            else_body: Some(vec![return_statement(
                                self.bool_expr(false, self.boolean, span),
                                span,
                            )]),
                        }),
                        span,
                    };
                    arms.push(hir::WhenArm {
                        pattern: hir::Pattern::Variant {
                            application,
                            variant: variant_index as u32,
                            fields: left_fields,
                        },
                        guard: None,
                        body: vec![inner],
                        span,
                    });
                }
                vec![hir::Statement {
                    kind: hir::StatementKind::When(hir::When {
                        subject: this_expr,
                        arms,
                        else_body: None,
                    }),
                    span,
                }]
            }
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
        match self.types[ty].clone() {
            Type::Unit | Type::Tuple(_) => {
                let (_, application) =
                    self.ensure_structural_derived_equality_application(ty, span, stack)?;
                Ok(self.derived_call(lhs, rhs, application, self.boolean, span))
            }
            Type::Struct(application) => {
                let function =
                    self.structs[self.struct_applications[application].template].derived_equality;
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
                                .resolve_explicit_field_equality(ty, lhs, rhs, path, span)
                                .map_err(|_| reason);
                        }
                    }
                }
                self.resolve_explicit_field_equality(ty, lhs, rhs, path, span)
            }
            Type::Enum(application) => {
                let function =
                    self.enums[self.enum_applications[application].template].derived_equality;
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
                                .resolve_explicit_field_equality(ty, lhs, rhs, path, span)
                                .map_err(|_| reason);
                        }
                    }
                }
                self.resolve_explicit_field_equality(ty, lhs, rhs, path, span)
            }
            _ => self.resolve_explicit_field_equality(ty, lhs, rhs, path, span),
        }
    }

    fn resolve_explicit_field_equality(
        &mut self,
        ty: hir::TypeId,
        lhs: hir::Expr,
        rhs: hir::Expr,
        path: &str,
        span: ast::Span,
    ) -> Result<hir::Expr, String> {
        let candidates = self
            .methods_by_name(ty, "equals")
            .into_iter()
            .filter(|candidate| {
                self.signatures[&candidate.function].operator == Some(hir::OperatorKind::Equals)
            })
            .collect::<Vec<_>>();
        let mut applicable = Vec::new();
        for candidate in candidates {
            let arguments = self.callable_candidate_owner_arguments(&candidate);
            let signature = self.signatures[&candidate.function].clone();
            debug_assert_eq!(
                signature.type_params.len(),
                signature.owner_type_param_count
            );
            let parameter = self.instantiate_ty(signature.params[0].ty, &arguments);
            if self.is_subtype(rhs.ty, parameter) {
                applicable.push((candidate, arguments, parameter));
            }
        }
        let mut winners = Vec::new();
        for index in 0..applicable.len() {
            let parameter = applicable[index].2;
            let dominates_all = (0..applicable.len())
                .all(|other| index == other || self.is_subtype(parameter, applicable[other].2));
            if dominates_all {
                winners.push(index);
            }
        }
        let [winner] = winners.as_slice() else {
            let reason = if applicable.is_empty() {
                format!(
                    "field `{path}` of type `{}` has no applicable member operator `equals`",
                    self.type_name(ty)
                )
            } else {
                format!(
                    "field `{path}` has an ambiguous member operator `equals` for type `{}`",
                    self.type_name(ty)
                )
            };
            return Err(reason);
        };
        let (candidate, arguments, parameter) = applicable.swap_remove(*winner);
        let callable = self.materialize_candidate_callable(&candidate, &arguments);
        let callee = self.materialize_method_callee(candidate.source, callable, &arguments);
        Ok(hir::Expr {
            kind: hir::ExprKind::MethodCall {
                receiver: Box::new(lhs),
                callee,
                args: vec![self.adapt_to(rhs, parameter)],
            },
            ty: self.boolean,
            span,
            origin: self.expression_origin(span),
        })
    }

    fn alloc_derived_local(
        &mut self,
        locals: &mut Arena<hir::Local>,
        name: &str,
        ty: hir::TypeId,
    ) -> hir::LocalId {
        locals.alloc(hir::Local {
            binding: self.fresh_binding(),
            name: name.to_string(),
            ty,
            mutable: false,
        })
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
