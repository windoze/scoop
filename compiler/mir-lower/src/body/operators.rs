use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_binary(
        &mut self,
        op: hir::BinOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        span: Span,
    ) -> smir::Expr {
        use mir::BinOp::*;
        match op {
            // String `+` is runtime concatenation (DESIGN 2.3); hir-lower
            // type checking makes both operands String here.
            hir::BinOp::Add if matches!(self.module.types[lhs.ty].kind, hir::TypeKind::String) => {
                self.call(
                    mir::Callee::Runtime(mir::RuntimeFn::StringConcat),
                    &[lhs, rhs],
                    mir::Type::String,
                )
            }
            hir::BinOp::Add => self.primitive(IntAdd, lhs, rhs),
            hir::BinOp::Sub => self.primitive(IntSub, lhs, rhs),
            hir::BinOp::Mul => self.primitive(IntMul, lhs, rhs),
            // M8 (DESIGN section 1): integer division checks the
            // divisor — a zero divisor throws `ArithmeticException`
            // instead of hitting LLVM `sdiv` UB. Both operands are
            // evaluated once into hidden locals (left to right), so
            // the check and the division share one evaluation.
            hir::BinOp::Div | hir::BinOp::Rem => {
                let lhs_slot = self.new_hidden("div", mir::Type::Int, false);
                let rhs_slot = self.new_hidden("div", mir::Type::Int, false);
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
                let throw =
                    self.throw_builtin(self.module.exception_core.arithmetic_exception, span);
                self.prelude.push(smir::StatementKind::If {
                    cond: smir::Expr::new(
                        mir::Type::Boolean,
                        smir::ExprKind::Binary {
                            op: mir::BinOp::IntEq,
                            lhs: Box::new(smir::Expr::local(rhs_slot, mir::Type::Int)),
                            rhs: Box::new(smir::Expr::int(0)),
                        },
                    ),
                    then_body: vec![throw],
                    else_body: None,
                });
                smir::Expr::new(
                    mir::Type::Int,
                    smir::ExprKind::Binary {
                        op: if op == hir::BinOp::Div {
                            IntDiv
                        } else {
                            IntRem
                        },
                        lhs: Box::new(smir::Expr::local(lhs_slot, mir::Type::Int)),
                        rhs: Box::new(smir::Expr::local(rhs_slot, mir::Type::Int)),
                    },
                )
            }
            hir::BinOp::Lt => self.primitive(IntLt, lhs, rhs),
            hir::BinOp::Le => self.primitive(IntLe, lhs, rhs),
            hir::BinOp::Gt => self.primitive(IntGt, lhs, rhs),
            hir::BinOp::Ge => self.primitive(IntGe, lhs, rhs),
            hir::BinOp::Eq | hir::BinOp::Ne => {
                unreachable!("HIR resolves == and != to exact method calls")
            }
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
            | mir::BinOp::BoolNe => mir::Type::Boolean,
            mir::BinOp::IntAdd
            | mir::BinOp::IntSub
            | mir::BinOp::IntMul
            | mir::BinOp::IntDiv
            | mir::BinOp::IntRem => self.lower_type(lhs.ty),
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
