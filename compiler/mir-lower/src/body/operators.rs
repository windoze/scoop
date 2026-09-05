use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_binary(
        &mut self,
        op: hir::BinOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> smir::Expr {
        use mir::BinOp::*;
        match op {
            hir::BinOp::Lt => self.primitive(IntLt, lhs, rhs),
            hir::BinOp::Le => self.primitive(IntLe, lhs, rhs),
            hir::BinOp::Gt => self.primitive(IntGt, lhs, rhs),
            hir::BinOp::Ge => self.primitive(IntGe, lhs, rhs),
            // `===` / `!==`: reference identity — the primitive
            // comparison on the two pointers.
            hir::BinOp::RefEq => self.primitive(IntEq, lhs, rhs),
            hir::BinOp::RefNe => self.primitive(IntNe, lhs, rhs),
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
        span: Span,
    ) -> smir::Expr {
        use hir::PrimitiveBinaryKind as K;
        use mir::BinOp as M;
        match kind {
            K::StringConcat => self.call(
                mir::Callee::Runtime(mir::RuntimeFn::StringConcat),
                &[lhs, rhs],
                mir::Type::String,
            ),
            K::StringCompareTo => self.call(
                mir::Callee::Runtime(mir::RuntimeFn::StringCompare),
                &[lhs, rhs],
                mir::Type::Int,
            ),
            K::IntDiv | K::IntRem => self.checked_integer_division(
                if kind == K::IntDiv {
                    M::IntDiv
                } else {
                    M::IntRem
                },
                lhs,
                rhs,
                mir::Type::Int,
                span,
            ),
            K::UIntDiv | K::UIntRem => self.checked_integer_division(
                if kind == K::UIntDiv {
                    M::UIntDiv
                } else {
                    M::UIntRem
                },
                lhs,
                rhs,
                mir::Type::UInt,
                span,
            ),
            K::IntAdd | K::UIntAdd => self.primitive(M::IntAdd, lhs, rhs),
            K::IntSub | K::UIntSub => self.primitive(M::IntSub, lhs, rhs),
            K::IntMul | K::UIntMul => self.primitive(M::IntMul, lhs, rhs),
            K::IntCompareTo => self.primitive(M::IntCompareTo, lhs, rhs),
            K::UIntCompareTo => self.primitive(M::UIntCompareTo, lhs, rhs),
        }
    }

    pub(super) fn lower_primitive_unary(
        &mut self,
        kind: hir::PrimitiveUnaryKind,
        operand: &hir::Expr,
    ) -> smir::Expr {
        use hir::PrimitiveUnaryKind as K;
        match kind {
            K::IntUnaryPlus | K::UIntUnaryPlus => self.lower_expr(operand),
            K::IntUnaryMinus => {
                let operand = Box::new(self.lower_expr(operand));
                smir::Expr::new(
                    mir::Type::Int,
                    smir::ExprKind::Unary {
                        op: mir::UnOp::IntNeg,
                        operand,
                    },
                )
            }
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
            K::IntInc | K::IntDec | K::UIntInc | K::UIntDec => {
                let ty = if matches!(kind, K::IntInc | K::IntDec) {
                    mir::Type::Int
                } else {
                    mir::Type::UInt
                };
                let op = if matches!(kind, K::IntInc | K::UIntInc) {
                    mir::BinOp::IntAdd
                } else {
                    mir::BinOp::IntSub
                };
                smir::Expr::new(
                    ty.clone(),
                    smir::ExprKind::Binary {
                        op,
                        lhs: Box::new(self.lower_expr(operand)),
                        rhs: Box::new(smir::Expr::new(ty, smir::ExprKind::IntLiteral(1))),
                    },
                )
            }
        }
    }

    fn checked_integer_division(
        &mut self,
        op: mir::BinOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        ty: mir::Type,
        span: Span,
    ) -> smir::Expr {
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
        let throw = self.throw_builtin(self.module.exception_core.arithmetic_exception, span);
        self.prelude.push(smir::StatementKind::If {
            cond: smir::Expr::new(
                mir::Type::Boolean,
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntEq,
                    lhs: Box::new(smir::Expr::local(rhs_slot, ty.clone())),
                    rhs: Box::new(smir::Expr::new(ty.clone(), smir::ExprKind::IntLiteral(0))),
                },
            ),
            then_body: vec![throw],
            else_body: None,
        });
        smir::Expr::new(
            ty.clone(),
            smir::ExprKind::Binary {
                op,
                lhs: Box::new(smir::Expr::local(lhs_slot, ty.clone())),
                rhs: Box::new(smir::Expr::local(rhs_slot, ty)),
            },
        )
    }

    pub(super) fn primitive(
        &mut self,
        op: mir::BinOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> smir::Expr {
        let ty = match op {
            mir::BinOp::IntLt
            | mir::BinOp::IntLe
            | mir::BinOp::IntGt
            | mir::BinOp::IntGe
            | mir::BinOp::IntEq
            | mir::BinOp::IntNe
            | mir::BinOp::BoolEq
            | mir::BinOp::BoolNe
            | mir::BinOp::MachineEq(_) => mir::Type::Boolean,
            mir::BinOp::IntAdd
            | mir::BinOp::IntSub
            | mir::BinOp::IntMul
            | mir::BinOp::IntDiv
            | mir::BinOp::IntRem
            | mir::BinOp::UIntDiv
            | mir::BinOp::UIntRem => self.lower_type(lhs.ty),
            mir::BinOp::IntCompareTo | mir::BinOp::UIntCompareTo => mir::Type::Int,
        };
        let lhs = Box::new(self.lower_expr(lhs));
        let rhs = Box::new(self.lower_expr(rhs));
        smir::Expr::new(ty, smir::ExprKind::Binary { op, lhs, rhs })
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

    /// Produce a pattern subject's nested value. Variant field extractions are
    /// guarded by the decision sequence that proved the active tag.
    pub(super) fn accessed(&mut self, root: mir::LocalId, path: &[Access]) -> smir::Expr {
        let mut current_ty = self.locals[root].ty.clone();
        let mut lowered = smir::Expr::local(root, current_ty.clone());
        for access in path {
            let (next_ty, kind) = match access {
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
                    (next_ty, kind)
                }
                Access::EnumField { variant, index } => {
                    let mir::Type::Enum(enum_id, _) = current_ty else {
                        unreachable!("variant pattern field access has an enum receiver")
                    };
                    let next_ty = self.enums.defs[enum_id].variants[*variant as usize].fields
                        [*index as usize]
                        .ty
                        .clone();
                    let kind = smir::ExprKind::EnumField {
                        operand: Box::new(lowered),
                        variant: *variant,
                        index: *index,
                    };
                    (next_ty, kind)
                }
            };
            current_ty = next_ty.clone();
            lowered = smir::Expr::new(next_ty, kind);
        }
        lowered
    }
}
