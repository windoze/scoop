use super::*;

mod primitive;

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
