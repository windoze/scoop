use super::*;

mod primitive;
mod references;

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
        let prelude_start = self.prelude.len();
        let span = rhs.span;
        let rhs = self.lower_expr(rhs);
        let setup = self.prelude.split_off(prelude_start);
        if setup.is_empty() {
            return logic(op, lhs, rhs);
        }

        // Runtime checks expanded while lowering the RHS must be guarded by
        // the same short-circuit edge as the expression that uses their values.
        let result = self.new_hidden("logic", mir::Type::Boolean, true);
        let assign = |value| smir::Statement {
            debug_origin: Default::default(),
            kind: smir::StatementKind::Assign {
                local: result,
                value,
            },
            span,
        };
        let mut rhs_body: Vec<_> = setup
            .into_iter()
            .map(|kind| smir::Statement {
                debug_origin: Default::default(),
                kind,
                span,
            })
            .collect();
        rhs_body.push(assign(rhs));
        let short_value = matches!(op, smir::LogicOp::Or);
        let short_body = vec![assign(smir::Expr::bool(short_value))];
        let (then_body, else_body) = if short_value {
            (short_body, rhs_body)
        } else {
            (rhs_body, short_body)
        };
        self.prelude.push(smir::StatementKind::If {
            cond: lhs,
            then_body,
            else_body: Some(else_body),
        });
        smir::Expr::local(result, mir::Type::Boolean)
    }

    /// Produce a pattern subject's nested value. Variant payload projections
    /// are guarded by the decision sequence that proved the active variant.
    pub(super) fn accessed(&mut self, root: mir::LocalId, path: &[Access]) -> smir::Expr {
        let mut lowered = smir::Expr::local(root, self.locals[root].ty.clone());
        for access in path {
            match access {
                Access::Field { index, ty: next_ty } => {
                    let kind = smir::ExprKind::FieldAccess {
                        receiver: Box::new(lowered),
                        index: *index,
                    };
                    lowered = smir::Expr::new(next_ty.clone(), kind);
                }
                Access::VariantField(field) => {
                    lowered =
                        smir::Expr::variant_payload_project(&self.enums.defs, lowered, *field);
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
