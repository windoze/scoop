use super::*;

impl BodyLowerer<'_> {
    /// Register a required box; finalization supplies its complete HIR conformance.
    pub(super) fn register_boxed(
        &mut self,
        payload: &mir::Type,
        payload_identity: hir::PersistentExactTypeId,
    ) {
        if is_boxable(payload) {
            self.boxed
                .get_or_create(self.classes, self.shell, payload, payload_identity);
        }
    }

    /// Register the boxed value type an `is` / `as` check needs (the
    /// runtime compares against the boxed type's TypeDescriptor).
    pub(super) fn register_check(&mut self, check_ty: &mir::Type, exact: hir::TypeId) {
        if is_boxable(check_ty) {
            let check_ty = check_ty.clone();
            self.register_boxed(&check_ty, self.module.exact_type_identities[exact].id());
        } else if let mir::Type::Function(function_type) = check_ty
            && !self.function_bridge_targets.contains(function_type)
        {
            self.function_bridge_targets.push(*function_type);
        }
    }

    /// `as` / `as?` (DESIGN 2.3): the operand is evaluated once into
    /// a hidden local. `as` throws `ClassCastException` when the
    /// runtime check fails (M8); `as?` wraps the result
    /// in `Some` / `None` through core's `Option` — the same prelude
    /// mechanism `!!` uses. A target of `Any` is statically true and
    /// needs no check. Class / interface targets stay the same
    /// reference; value targets come out of the box (`Unbox`).
    pub(super) fn lower_cast(
        &mut self,
        operand: &hir::Expr,
        target_hir: hir::TypeId,
        optional: bool,
        expr_ty: hir::TypeId,
        span: Span,
    ) -> smir::Expr {
        let target = self.lower_type(target_hir);
        self.register_check(&target, target_hir);
        let operand_ty = self.lower_type(operand.ty);
        let value = self.lower_expr(operand);
        let slot = self.new_hidden("cast", operand_ty.clone(), false);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: slot,
            init: value,
        });
        let cond = match &target {
            mir::Type::Any => smir::Expr::bool(true),
            _ => smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::IsInstance {
                    operand: Box::new(smir::Expr::local(slot, operand_ty.clone())),
                    check_ty: Box::new(target.clone()),
                },
            ),
        };
        if !optional {
            let throw = match self.core_protocols {
                hir::ConcreteCoreProtocols::Defined(protocols) => {
                    self.throw_builtin(protocols.exceptions.class_cast_exception, span)
                }
                hir::ConcreteCoreProtocols::Imported(protocols) => self.throw_imported_exception(
                    protocols.exceptions().class_cast_exception().persistent(),
                    protocols.exceptions().class_cast_exception_constructor(),
                    span,
                ),
            };
            self.prelude.push(smir::StatementKind::If {
                cond: smir::Expr::new(
                    mir::Type::Boolean,
                    smir::ExprKind::Unary {
                        op: mir::UnOp::BoolNot,
                        operand: Box::new(cond),
                    },
                ),
                then_body: vec![throw],
                else_body: None,
            });
            // A checked function view needs an exact target-ABI closure;
            // its invoke calls the source closure's fixed dynamic invoke.
            // Value targets are unboxed by the outer HIR node.
            return match target {
                mir::Type::Function(function_type) => self.adapt_checked_function_value(
                    smir::Expr::local(slot, operand_ty),
                    function_type,
                    span,
                ),
                target @ (mir::Type::String
                | mir::Type::Class(_)
                | mir::Type::Interface(_)
                | mir::Type::Any) => smir::Expr::new(
                    target.clone(),
                    smir::ExprKind::Retype {
                        operand: Box::new(smir::Expr::local(slot, operand_ty)),
                        ty: Box::new(target),
                    },
                ),
                // hir-lower wraps a checked value-type cast in an outer
                // `Unbox`. Preserve the checked boxed/reference operand here;
                // the outer node is the sole operation that produces the
                // target value type.
                _ => smir::Expr::local(slot, operand_ty),
            };
        }
        let unboxed = match &target {
            mir::Type::Function(function_type) => self.adapt_checked_function_value(
                smir::Expr::local(slot, operand_ty.clone()),
                *function_type,
                span,
            ),
            mir::Type::String | mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::Any => {
                smir::Expr::new(
                    target.clone(),
                    smir::ExprKind::Retype {
                        operand: Box::new(smir::Expr::local(slot, operand_ty.clone())),
                        ty: Box::new(target.clone()),
                    },
                )
            }
            // Only `as?` unwraps here: hir-lower wraps a value-typed
            // `as` in a hir-level `Unbox(Cast)` node, so the payload
            // extraction for `as` happens when that outer `Unbox` is
            // lowered — adding another one here would double-unwrap.
            _ => smir::Expr::new(
                target.clone(),
                smir::ExprKind::Unbox(Box::new(smir::Expr::local(slot, operand_ty))),
            ),
        };
        let option_ty = self.lower_type(expr_ty);
        let option = option_core_for_type(self.module, self.enums, &option_ty);
        let (some, none) = (option.some(), option.none());
        let result = self.new_hidden("cast", option_ty.clone(), true);
        let some_value = smir::Expr::new(
            option_ty.clone(),
            smir::ExprKind::VariantConstruct {
                variant: some,
                fields: vec![unboxed],
            },
        );
        let none_value = smir::Expr::new(
            option_ty.clone(),
            smir::ExprKind::VariantConstruct {
                variant: none,
                fields: Vec::new(),
            },
        );
        self.prelude.push(smir::StatementKind::If {
            cond,
            then_body: vec![smir::Statement {
                kind: smir::StatementKind::Assign {
                    local: result,
                    value: some_value,
                },
                span,
            }],
            else_body: Some(vec![smir::Statement {
                kind: smir::StatementKind::Assign {
                    local: result,
                    value: none_value,
                },
                span,
            }]),
        });
        smir::Expr::local(result, option_ty)
    }
}
