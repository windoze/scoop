use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_binary(
        &mut self,
        op: hir::BinOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> smir::Expr {
        match op {
            hir::BinOp::Lt | hir::BinOp::Le | hir::BinOp::Gt | hir::BinOp::Ge => {
                let kind = mir::IntegerKind::SIGNED_64;
                assert_eq!(
                    self.lower_type(lhs.ty),
                    mir::Type::Integer(kind),
                    "ordered HIR comparison consumes the canonical Long compareTo result"
                );
                assert_eq!(
                    self.lower_type(rhs.ty),
                    mir::Type::Integer(kind),
                    "ordered HIR comparison compares the Long result with Long zero"
                );
                let operator = match op {
                    hir::BinOp::Lt => mir::IntegerComparisonOperator::LessThan,
                    hir::BinOp::Le => mir::IntegerComparisonOperator::LessThanOrEqual,
                    hir::BinOp::Gt => mir::IntegerComparisonOperator::GreaterThan,
                    hir::BinOp::Ge => mir::IntegerComparisonOperator::GreaterThanOrEqual,
                    _ => unreachable!("the outer match selects an ordered comparison"),
                };
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                smir::Expr::integer_compare(
                    mir::IntegerComparisonOperation::new(kind, operator),
                    lhs,
                    rhs,
                )
            }
            // `===` / `!==`: reference identity — the primitive
            // comparison on the two pointers.
            hir::BinOp::RefEq | hir::BinOp::RefNe => smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: if op == hir::BinOp::RefEq {
                        mir::BinOp::RefEq
                    } else {
                        mir::BinOp::RefNe
                    },
                    lhs: Box::new(self.lower_expr(lhs)),
                    rhs: Box::new(self.lower_expr(rhs)),
                },
            ),
            // Short-circuit operators stay in the private construction
            // tree until CFG normalization emits their branch edges.
            hir::BinOp::And => self.short_circuit(smir::LogicOp::And, lhs, rhs),
            hir::BinOp::Or => self.short_circuit(smir::LogicOp::Or, lhs, rhs),
        }
    }

    pub(super) fn lower_primitive_binary(
        &mut self,
        kind: hir::PrimitiveBinaryKind,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> smir::Expr {
        use hir::PrimitiveBinaryKind as K;
        match kind {
            K::StringConcat => self.call(
                mir::Callee::Runtime(mir::RuntimeFn::StringConcat),
                &[lhs, rhs],
                mir::Type::String,
            ),
            K::StringCompareTo => self.call(
                mir::Callee::Runtime(mir::RuntimeFn::StringCompare),
                &[lhs, rhs],
                mir::Type::Integer(mir::IntegerKind::SIGNED_64),
            ),
        }
    }

    pub(super) fn lower_primitive_unary(
        &mut self,
        kind: hir::PrimitiveUnaryKind,
        operand: &hir::Expr,
    ) -> smir::Expr {
        use hir::PrimitiveUnaryKind as K;
        match kind {
            K::BooleanNot => {
                let operand = Box::new(self.lower_expr(operand));
                smir::Expr::new(
                    mir::Type::Boolean,
                    smir::ExprKind::Unary {
                        op: mir::UnOp::BoolNot,
                        operand,
                    },
                )
            }
        }
    }

    pub(super) fn lower_integer_operation(
        &mut self,
        operation: &hir::HirIntegerOperation,
        arguments: &hir::HirIntegerOperationArguments,
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
                        let hir::HirIntegerOperationArguments::Unary(operand) = arguments else {
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
                        let operand = self.lower_expr(operand);
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
                        let hir::HirIntegerOperationArguments::Binary { lhs, rhs } = arguments
                        else {
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
                        let lhs = self.lower_expr(lhs);
                        let rhs = self.lower_expr(rhs);
                        smir::Expr::integer_binary(
                            mir::IntegerBinaryOperation::new(kind, operator),
                            lhs,
                            rhs,
                        )
                    }
                    hir::NoGcIntegerOperation::CompareTo => {
                        let hir::HirIntegerOperationArguments::Binary { lhs, rhs } = arguments
                        else {
                            unreachable!("validated compareTo has two operands")
                        };
                        let lhs = self.lower_expr(lhs);
                        let rhs = self.lower_expr(rhs);
                        smir::Expr::integer_compare_to(
                            mir::IntegerCompareToOperation::new(kind),
                            lhs,
                            rhs,
                        )
                    }
                    hir::NoGcIntegerOperation::Equals => {
                        let hir::HirIntegerOperationArguments::Binary { lhs, rhs } = arguments
                        else {
                            unreachable!("validated integer equals has two operands")
                        };
                        let lhs = self.lower_expr(lhs);
                        let rhs = self.lower_expr(rhs);
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
                let hir::HirIntegerOperationArguments::Binary { lhs, rhs } = arguments else {
                    unreachable!("validated integer div/rem has two operands")
                };
                let operator = match operation {
                    hir::IntegerDivRem::Div => mir::SafeIntegerDivRemOperator::Divide,
                    hir::IntegerDivRem::Rem => mir::SafeIntegerDivRemOperator::Remainder,
                };
                self.checked_integer_division(lower_integer_kind(*kind), operator, lhs, rhs, span)
            }
        }
    }

    pub(super) fn lower_integer_conversion(
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
        arguments: &hir::HirIntegerOperationArguments,
    ) -> smir::Expr {
        let hir::HirIntegerOperationArguments::Binary { lhs, rhs } = arguments else {
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
        let value = self.lower_expr(lhs);
        let count = self.lower_expr(rhs);
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
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        span: Span,
    ) -> smir::Expr {
        let ty = mir::Type::Integer(kind);
        let lhs_slot = self.new_hidden("div", ty.clone(), false);
        let rhs_slot = self.new_hidden("div", ty.clone(), false);
        let lhs = self.lower_expr(lhs);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: lhs_slot,
            init: lhs,
        });
        let rhs = self.lower_expr(rhs);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: rhs_slot,
            init: rhs,
        });
        let throw = self.throw_builtin(self.core_protocols.exceptions.arithmetic_exception, span);
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

    pub(super) fn short_circuit(
        &mut self,
        op: smir::LogicOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> smir::Expr {
        let lhs = self.lower_expr(lhs);
        let rhs = self.lower_expr(rhs);
        logic(op, lhs, rhs)
    }

    /// Produce a pattern subject's nested value. Variant payload projections
    /// are guarded by the decision sequence that proved the active variant.
    pub(super) fn accessed(&mut self, root: mir::LocalId, path: &[Access]) -> smir::Expr {
        let mut current_ty = self.locals[root].ty.clone();
        let mut lowered = smir::Expr::local(root, current_ty.clone());
        for access in path {
            match access {
                Access::Field(index) => {
                    let next_ty = match &current_ty {
                        mir::Type::Tuple(elements) => elements[*index as usize].clone(),
                        mir::Type::Struct(struct_id) => self.structs.defs[*struct_id]
                            .declared_fields()[*index as usize]
                            .ty
                            .clone(),
                        _ => unreachable!("tuple/struct patterns only access aggregate fields"),
                    };
                    let kind = smir::ExprKind::FieldAccess {
                        receiver: Box::new(lowered),
                        index: *index,
                    };
                    current_ty = next_ty.clone();
                    lowered = smir::Expr::new(next_ty, kind);
                }
                Access::VariantField(field) => {
                    lowered =
                        smir::Expr::variant_payload_project(&self.enums.defs, lowered, *field);
                    current_ty = lowered.ty.clone();
                }
            }
        }
        lowered
    }
}

fn integer_constant(kind: mir::IntegerKind, raw_bits: u64) -> smir::Expr {
    let value = mir::MirIntegerConstant::from_raw_bits(kind, raw_bits)
        .expect("synthetic integer constants fit their exact width");
    smir::Expr::integer(value)
}
