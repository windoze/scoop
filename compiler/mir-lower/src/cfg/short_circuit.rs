use super::*;

impl CfgLowerer<'_> {
    pub(super) fn lower_short_circuit(
        &mut self,
        lhs: &smir::Expr,
        rhs: &smir::Expr,
        short_value: bool,
        span: Span,
    ) -> Option<mir::Expr> {
        let lhs = self.lower_expr(lhs, span)?;
        let rhs_block = self.new_block("logic.rhs");
        let short_block = self.new_block("logic.short");
        let merge = self.new_block("logic.merge");
        let (then_block, else_block) = if short_value {
            (short_block, rhs_block)
        } else {
            (rhs_block, short_block)
        };
        self.seal(mir::Terminator::Branch {
            cond: lhs,
            then_block,
            else_block,
        });

        let result = self.new_hidden(
            "logic",
            StructuralDefinitionSiteRole::SyntheticValue,
            SyntheticLocalRole::Temporary,
            mir::Type::Boolean,
        );
        self.enter(short_block);
        self.push(
            mir::StatementKind::Assign {
                local: result,
                value: mir::Expr::new(mir::Type::Boolean, mir::ExprKind::BoolLiteral(short_value)),
            },
            span,
        );
        self.seal(mir::Terminator::Goto(merge));

        self.enter(rhs_block);
        if let Some(rhs) = self.lower_expr(rhs, span) {
            self.push(
                mir::StatementKind::Assign {
                    local: result,
                    value: rhs,
                },
                span,
            );
            self.seal(mir::Terminator::Goto(merge));
        }

        self.enter(merge);
        Some(mir::Expr::new(
            mir::Type::Boolean,
            mir::ExprKind::Local(result),
        ))
    }
}
