use super::*;

enum IntegerValues {
    Unary(smir::Expr),
    Binary { lhs: smir::Expr, rhs: smir::Expr },
}

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

    pub(crate) fn lower_primitive_member_values(
        &mut self,
        intrinsic: scoop_hir::PrimitiveMemberIntrinsic,
        args: Vec<smir::Expr>,
        result_type: &mir::Type,
        span: Span,
    ) -> smir::Expr {
        use scoop_hir::PrimitiveMemberIntrinsic as I;
        match intrinsic {
            I::Char(kind) => super::characters::character_member(kind, args, result_type),
            I::Unary(kind) => {
                let [operand] = args.try_into().expect("a unary member has one receiver");
                primitive_unary_value(kind, operand)
            }
            I::Binary(kind) => {
                let [lhs, rhs] = args.try_into().expect("a binary member has two operands");
                primitive_binary_values(kind, lhs, rhs)
            }
            I::Integer(kind) => {
                let operation = match kind {
                    scoop_hir::IntegerIntrinsicKind::NoGcOperation { kind, operation } => {
                        hir::IntegerOperation::NoGc { kind, operation }
                    }
                    scoop_hir::IntegerIntrinsicKind::ManagedOperation { kind, operation } => {
                        hir::IntegerOperation::Managed { kind, operation }
                    }
                    scoop_hir::IntegerIntrinsicKind::Conversion {
                        source,
                        target_kind,
                    } => {
                        let [operand] = args.try_into().expect("a conversion has one receiver");
                        return smir::Expr::integer_conversion(
                            mir::IntegerConversion::new(
                                lower_integer_kind(source),
                                lower_integer_kind(target_kind),
                            ),
                            operand,
                        );
                    }
                };
                let arguments = match operation.arity() {
                    hir::IntegerOperationArity::Unary => {
                        let [operand] = args.try_into().expect("a unary member has one receiver");
                        IntegerValues::Unary(operand)
                    }
                    hir::IntegerOperationArity::Binary => {
                        let [lhs, rhs] = args.try_into().expect("a binary member has two operands");
                        IntegerValues::Binary { lhs, rhs }
                    }
                };
                self.lower_integer_values(&operation, arguments, span)
            }
        }
    }

    pub(in crate::body) fn lower_integer_operation(
        &mut self,
        operation: &hir::HirIntegerOperation,
        arguments: &hir::HirIntegerOperationArguments,
        span: Span,
    ) -> smir::Expr {
        if let hir::IntegerOperation::Managed { kind, operation } = operation {
            let hir::HirIntegerOperationArguments::Binary { lhs, rhs } = arguments else {
                unreachable!("validated integer div/rem has two operands");
            };
            return self.checked_integer_division(
                lower_integer_kind(*kind),
                div_rem_operator(*operation),
                |this| this.lower_expr(lhs),
                |this| this.lower_expr(rhs),
                span,
            );
        }
        let arguments = match arguments {
            hir::HirIntegerOperationArguments::Unary(value) => {
                IntegerValues::Unary(self.lower_expr(value))
            }
            hir::HirIntegerOperationArguments::Binary { lhs, rhs } => IntegerValues::Binary {
                lhs: self.lower_expr(lhs),
                rhs: self.lower_expr(rhs),
            },
        };
        self.lower_integer_values(operation, arguments, span)
    }

    fn lower_integer_values(
        &mut self,
        operation: &hir::IntegerOperation,
        arguments: IntegerValues,
        span: Span,
    ) -> smir::Expr {
        match operation {
            hir::IntegerOperation::NoGc {
                kind, operation, ..
            } => {
                let kind = lower_integer_kind(*kind);
                match operation {
                    hir::NoGcIntegerOperation::UnaryPlus
                    | hir::NoGcIntegerOperation::UnaryMinus
                    | hir::NoGcIntegerOperation::Inc
                    | hir::NoGcIntegerOperation::Dec
                    | hir::NoGcIntegerOperation::Inv => {
                        let IntegerValues::Unary(operand) = arguments else {
                            unreachable!("validated unary integer operation has one receiver")
                        };
                        let operator = match operation {
                            hir::NoGcIntegerOperation::UnaryPlus => {
                                mir::IntegerUnaryOperator::Identity
                            }
                            hir::NoGcIntegerOperation::UnaryMinus => {
                                mir::IntegerUnaryOperator::Negate
                            }
                            hir::NoGcIntegerOperation::Inc => mir::IntegerUnaryOperator::Increment,
                            hir::NoGcIntegerOperation::Dec => mir::IntegerUnaryOperator::Decrement,
                            hir::NoGcIntegerOperation::Inv => mir::IntegerUnaryOperator::BitNot,
                            _ => unreachable!("the outer match selects a unary operation"),
                        };
                        smir::Expr::integer_unary(
                            mir::IntegerUnaryOperation::new(kind, operator),
                            operand,
                        )
                    }
                    hir::NoGcIntegerOperation::Add
                    | hir::NoGcIntegerOperation::Sub
                    | hir::NoGcIntegerOperation::Mul
                    | hir::NoGcIntegerOperation::And
                    | hir::NoGcIntegerOperation::Or
                    | hir::NoGcIntegerOperation::Xor => {
                        let IntegerValues::Binary { lhs, rhs } = arguments else {
                            unreachable!("validated binary integer operation has two operands")
                        };
                        let operator = match operation {
                            hir::NoGcIntegerOperation::Add => mir::IntegerBinaryOperator::Add,
                            hir::NoGcIntegerOperation::Sub => mir::IntegerBinaryOperator::Subtract,
                            hir::NoGcIntegerOperation::Mul => mir::IntegerBinaryOperator::Multiply,
                            hir::NoGcIntegerOperation::And => mir::IntegerBinaryOperator::BitAnd,
                            hir::NoGcIntegerOperation::Or => mir::IntegerBinaryOperator::BitOr,
                            hir::NoGcIntegerOperation::Xor => mir::IntegerBinaryOperator::BitXor,
                            _ => unreachable!("the outer match selects a binary operation"),
                        };
                        smir::Expr::integer_binary(
                            mir::IntegerBinaryOperation::new(kind, operator),
                            lhs,
                            rhs,
                        )
                    }
                    hir::NoGcIntegerOperation::CompareTo => {
                        let IntegerValues::Binary { lhs, rhs } = arguments else {
                            unreachable!("validated compareTo has two operands")
                        };
                        smir::Expr::integer_compare_to(
                            mir::IntegerCompareToOperation::new(kind),
                            lhs,
                            rhs,
                        )
                    }
                    hir::NoGcIntegerOperation::Equals => {
                        let IntegerValues::Binary { lhs, rhs } = arguments else {
                            unreachable!("validated integer equals has two operands")
                        };
                        smir::Expr::integer_compare(
                            mir::IntegerComparisonOperation::new(
                                kind,
                                mir::IntegerComparisonOperator::Equal,
                            ),
                            lhs,
                            rhs,
                        )
                    }
                    hir::NoGcIntegerOperation::Shl
                    | hir::NoGcIntegerOperation::Shr
                    | hir::NoGcIntegerOperation::Ushr => {
                        self.lower_integer_shift(kind, *operation, arguments)
                    }
                }
            }
            hir::IntegerOperation::Managed {
                kind, operation, ..
            } => {
                let IntegerValues::Binary { lhs, rhs } = arguments else {
                    unreachable!("validated integer div/rem has two operands")
                };
                let operator = match operation {
                    hir::IntegerDivRem::Div => mir::SafeIntegerDivRemOperator::Divide,
                    hir::IntegerDivRem::Rem => mir::SafeIntegerDivRemOperator::Remainder,
                };
                self.checked_integer_division(
                    lower_integer_kind(*kind),
                    operator,
                    |_| lhs,
                    |_| rhs,
                    span,
                )
            }
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

    fn lower_integer_shift(
        &mut self,
        kind: mir::IntegerKind,
        source_operation: hir::NoGcIntegerOperation,
        arguments: IntegerValues,
    ) -> smir::Expr {
        let IntegerValues::Binary { lhs, rhs } = arguments else {
            unreachable!("validated integer shift has a value and Long count")
        };
        let operator = match source_operation {
            hir::NoGcIntegerOperation::Shl => mir::IntegerShiftOperator::Left,
            hir::NoGcIntegerOperation::Shr => mir::IntegerShiftOperator::Right,
            hir::NoGcIntegerOperation::Ushr => mir::IntegerShiftOperator::UnsignedRight,
            _ => unreachable!("only shifts enter shift lowering"),
        };
        let operation = mir::IntegerShiftOperation::new(kind, operator)
            .expect("HIR rejects unsigned ushr before MIR lowering");
        let value = lhs;
        let count = rhs;
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
                kind: smir::StatementKind::Assign {
                    local: result,
                    value: integer_constant(kind, special_bits),
                },
                span,
            }],
            else_body: Some(vec![smir::Statement {
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

fn primitive_unary_value(kind: hir::PrimitiveUnaryKind, operand: smir::Expr) -> smir::Expr {
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

fn primitive_binary_values(
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
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::Runtime(function),
            },
            args: vec![lhs, rhs],
            return_ty,
        }),
    )
}

fn div_rem_operator(operation: hir::IntegerDivRem) -> mir::SafeIntegerDivRemOperator {
    match operation {
        hir::IntegerDivRem::Div => mir::SafeIntegerDivRemOperator::Divide,
        hir::IntegerDivRem::Rem => mir::SafeIntegerDivRemOperator::Remainder,
    }
}
