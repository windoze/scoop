use super::*;

impl BodyLowerer<'_> {
    pub(in crate::body) fn lower_primitive_binary(
        &mut self,
        kind: hir::PrimitiveBinaryKind,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> smir::Expr {
        let lhs = self.lower_expr(lhs);
        let rhs = self.lower_expr(rhs);
        primitive_binary_values(kind, lhs, rhs)
    }

    pub(in crate::body) fn lower_primitive_unary(
        &mut self,
        kind: hir::PrimitiveUnaryKind,
        operand: &hir::Expr,
    ) -> smir::Expr {
        primitive_unary_value(kind, self.lower_expr(operand))
    }

    pub(in crate::body) fn lower_integer_operation(
        &mut self,
        expression: &hir::HirIntegerOperation,
        span: Span,
    ) -> smir::Expr {
        use hir::{IntegerBinaryOperator as B, IntegerExpression as E};
        match expression {
            E::Unary {
                kind,
                operation,
                operand,
            } => integer_unary_value(*kind, *operation, self.lower_expr(operand)),
            E::Binary {
                kind,
                operation: B::Div,
                lhs,
                rhs,
            } => self.checked_integer_division(
                lower_integer_kind(*kind),
                mir::SafeIntegerDivRemOperator::Divide,
                |this| this.lower_expr(lhs),
                |this| this.lower_expr(rhs),
                span,
            ),
            E::Binary {
                kind,
                operation: B::Rem,
                lhs,
                rhs,
            } => self.checked_integer_division(
                lower_integer_kind(*kind),
                mir::SafeIntegerDivRemOperator::Remainder,
                |this| this.lower_expr(lhs),
                |this| this.lower_expr(rhs),
                span,
            ),
            E::Binary {
                kind,
                operation,
                lhs,
                rhs,
            } => {
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                self.integer_binary_values(*kind, *operation, lhs, rhs, span)
            }
        }
    }

    pub(crate) fn integer_binary_values(
        &mut self,
        kind: hir::IntegerKind,
        operation: hir::IntegerBinaryOperator,
        lhs: smir::Expr,
        rhs: smir::Expr,
        span: Span,
    ) -> smir::Expr {
        use hir::IntegerBinaryOperator as B;
        use mir::IntegerBinaryOperator as M;
        let kind = lower_integer_kind(kind);
        match operation {
            B::Add => {
                smir::Expr::integer_binary(mir::IntegerBinaryOperation::new(kind, M::Add), lhs, rhs)
            }
            B::Sub => smir::Expr::integer_binary(
                mir::IntegerBinaryOperation::new(kind, M::Subtract),
                lhs,
                rhs,
            ),
            B::Mul => smir::Expr::integer_binary(
                mir::IntegerBinaryOperation::new(kind, M::Multiply),
                lhs,
                rhs,
            ),
            B::And => smir::Expr::integer_binary(
                mir::IntegerBinaryOperation::new(kind, M::BitAnd),
                lhs,
                rhs,
            ),
            B::Or => smir::Expr::integer_binary(
                mir::IntegerBinaryOperation::new(kind, M::BitOr),
                lhs,
                rhs,
            ),
            B::Xor => smir::Expr::integer_binary(
                mir::IntegerBinaryOperation::new(kind, M::BitXor),
                lhs,
                rhs,
            ),
            B::CompareTo => {
                smir::Expr::integer_compare_to(mir::IntegerCompareToOperation::new(kind), lhs, rhs)
            }
            B::Equals => smir::Expr::integer_compare(
                mir::IntegerComparisonOperation::new(kind, mir::IntegerComparisonOperator::Equal),
                lhs,
                rhs,
            ),
            B::Shl => integer_shift(kind, mir::IntegerShiftOperator::Left, lhs, rhs),
            B::Shr => integer_shift(kind, mir::IntegerShiftOperator::Right, lhs, rhs),
            B::Ushr => integer_shift(kind, mir::IntegerShiftOperator::UnsignedRight, lhs, rhs),
            B::Div => self.checked_integer_division(
                kind,
                mir::SafeIntegerDivRemOperator::Divide,
                |_| lhs,
                |_| rhs,
                span,
            ),
            B::Rem => self.checked_integer_division(
                kind,
                mir::SafeIntegerDivRemOperator::Remainder,
                |_| lhs,
                |_| rhs,
                span,
            ),
        }
    }

    pub(in crate::body) fn lower_integer_conversion(
        &mut self,
        conversion: &hir::HirIntegerConversion,
        operand: &hir::Expr,
    ) -> smir::Expr {
        let conversion = mir::IntegerConversion::new(
            lower_integer_kind(conversion.source),
            lower_integer_kind(conversion.target_kind),
        );
        let operand = self.lower_expr(operand);
        smir::Expr::integer_conversion(conversion, operand)
    }

    fn checked_integer_division(
        &mut self,
        kind: mir::IntegerKind,
        operator: mir::SafeIntegerDivRemOperator,
        lhs: impl FnOnce(&mut Self) -> smir::Expr,
        rhs: impl FnOnce(&mut Self) -> smir::Expr,
        span: Span,
    ) -> smir::Expr {
        let ty = mir::Type::Integer(kind);
        let lhs_slot = self.new_hidden("div", ty.clone(), false);
        let rhs_slot = self.new_hidden("div", ty.clone(), false);
        let lhs = lhs(self);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: lhs_slot,
            init: lhs,
        });
        let rhs = rhs(self);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: rhs_slot,
            init: rhs,
        });
        let throw = match self.core_protocols {
            hir::ConcreteCoreProtocols::Defined(protocols) => {
                self.throw_builtin(protocols.exceptions.arithmetic_exception, span)
            }
            hir::ConcreteCoreProtocols::Imported(protocols) => self.throw_imported_exception(
                protocols.exceptions().arithmetic_exception().persistent(),
                protocols.exceptions().arithmetic_exception_constructor(),
                span,
            ),
        };
        self.prelude.push(smir::StatementKind::If {
            cond: smir::Expr::integer_compare(
                mir::IntegerComparisonOperation::new(kind, mir::IntegerComparisonOperator::Equal),
                smir::Expr::local(rhs_slot, ty.clone()),
                integer_constant(kind, 0),
            ),
            then_body: vec![throw],
            else_body: None,
        });
        let ordinary = || {
            smir::Expr::safe_integer_div_rem(
                mir::SafeIntegerDivRemOperation::new(kind, operator),
                smir::Expr::local(lhs_slot, ty.clone()),
                smir::Expr::local(rhs_slot, ty.clone()),
            )
        };
        if kind.signedness() == mir::IntegerSignedness::Unsigned {
            return ordinary();
        }

        let sign_bit = 1_u64 << (kind.width().bits() - 1);
        let minus_one = kind.width().raw_mask();
        let boundary = and(
            smir::Expr::integer_compare(
                mir::IntegerComparisonOperation::new(kind, mir::IntegerComparisonOperator::Equal),
                smir::Expr::local(lhs_slot, ty.clone()),
                integer_constant(kind, sign_bit),
            ),
            smir::Expr::integer_compare(
                mir::IntegerComparisonOperation::new(kind, mir::IntegerComparisonOperator::Equal),
                smir::Expr::local(rhs_slot, ty.clone()),
                integer_constant(kind, minus_one),
            ),
        );
        let result = self.new_hidden("div.result", ty.clone(), false);
        let special_bits = match operator {
            mir::SafeIntegerDivRemOperator::Divide => sign_bit,
            mir::SafeIntegerDivRemOperator::Remainder => 0,
        };
        self.prelude.push(smir::StatementKind::If {
            cond: boundary,
            then_body: vec![smir::Statement {
                debug_origin: Default::default(),
                kind: smir::StatementKind::Assign {
                    local: result,
                    value: integer_constant(kind, special_bits),
                },
                span,
            }],
            else_body: Some(vec![smir::Statement {
                debug_origin: Default::default(),
                kind: smir::StatementKind::Assign {
                    local: result,
                    value: ordinary(),
                },
                span,
            }]),
        });
        smir::Expr::local(result, ty)
    }
}

pub(crate) fn integer_unary_value(
    kind: hir::IntegerKind,
    operation: hir::IntegerUnaryOperator,
    operand: smir::Expr,
) -> smir::Expr {
    use hir::IntegerUnaryOperator as H;
    let operator = match operation {
        H::UnaryPlus => mir::IntegerUnaryOperator::Identity,
        H::UnaryMinus => mir::IntegerUnaryOperator::Negate,
        H::Inc => mir::IntegerUnaryOperator::Increment,
        H::Dec => mir::IntegerUnaryOperator::Decrement,
        H::Inv => mir::IntegerUnaryOperator::BitNot,
    };
    smir::Expr::integer_unary(
        mir::IntegerUnaryOperation::new(lower_integer_kind(kind), operator),
        operand,
    )
}

fn integer_shift(
    kind: mir::IntegerKind,
    operator: mir::IntegerShiftOperator,
    value: smir::Expr,
    count: smir::Expr,
) -> smir::Expr {
    let operation = mir::IntegerShiftOperation::new(kind, operator)
        .expect("HIR rejects unsigned ushr before MIR lowering");
    let count_kind = mir::IntegerKind::SIGNED_64;
    let mask = u64::from(kind.width().bits() - 1);
    let normalized_count = smir::Expr::integer_binary(
        mir::IntegerBinaryOperation::new(count_kind, mir::IntegerBinaryOperator::BitAnd),
        count,
        integer_constant(count_kind, mask),
    );
    let normalized_count = smir::Expr::integer_conversion(
        mir::IntegerConversion::new(count_kind, kind),
        normalized_count,
    );
    smir::Expr::integer_shift(operation, value, normalized_count)
}

pub(crate) fn primitive_unary_value(
    kind: hir::PrimitiveUnaryKind,
    operand: smir::Expr,
) -> smir::Expr {
    match kind {
        hir::PrimitiveUnaryKind::BooleanNot => smir::Expr::new(
            mir::Type::Boolean,
            smir::ExprKind::Unary {
                op: mir::UnOp::BoolNot,
                operand: Box::new(operand),
            },
        ),
    }
}

pub(crate) fn primitive_binary_values(
    kind: hir::PrimitiveBinaryKind,
    lhs: smir::Expr,
    rhs: smir::Expr,
) -> smir::Expr {
    let (function, return_ty) = match kind {
        hir::PrimitiveBinaryKind::StringConcat => (mir::RuntimeFn::StringConcat, mir::Type::String),
        hir::PrimitiveBinaryKind::StringCompareTo => (
            mir::RuntimeFn::StringCompare,
            mir::Type::Integer(mir::IntegerKind::SIGNED_64),
        ),
    };
    smir::Expr::new(
        return_ty.clone(),
        smir::ExprKind::Call(smir::Call {
            target: mir::CallTarget::Direct(mir::Callee::Runtime(function)),
            args: vec![lhs, rhs],
            return_ty,
        }),
    )
}
