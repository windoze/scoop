use super::*;

impl<'a> CfgLowerer<'a> {
    /// Expand pattern tests as individual branches. Keeping each
    /// `VariantTest` directly in a branch terminator makes its true-edge proof
    /// available to every subsequent payload projection in the matched path.
    pub(super) fn lower_pattern_decision(
        &mut self,
        decision: &'a smir::PatternDecision,
        span: Span,
    ) {
        let test_count = decision
            .steps
            .iter()
            .filter(|step| matches!(step, smir::PatternDecisionStep::Test(_)))
            .count();
        assert!(test_count > 0, "a pattern decision contains a runtime test");

        let pass_blocks = (0..test_count)
            .map(|_| self.new_block("pattern.pass"))
            .collect::<Vec<_>>();
        let else_block = (!decision.else_body.is_empty()).then(|| self.new_block("pattern.else"));
        let merge_block = self.new_block("pattern.merge");
        let failure_block = else_block.unwrap_or(merge_block);
        let mut pass_blocks = pass_blocks.into_iter();

        for step in &decision.steps {
            match step {
                smir::PatternDecisionStep::Materialize { local, init } => {
                    assert!(
                        !matches!(init.kind, smir::ExprKind::Call(_)),
                        "pattern materialization does not defer a call"
                    );
                    let init = self.lower_expr(init, span);
                    self.push(
                        mir::StatementKind::ValDecl {
                            local: *local,
                            init,
                        },
                        span,
                    );
                }
                smir::PatternDecisionStep::Test(test) => {
                    let test = self.lower_expr(test, span);
                    assert_eq!(test.ty, mir::Type::Boolean, "pattern tests are Boolean");
                    let pass_block = pass_blocks
                        .next()
                        .expect("each pattern test has one success block");
                    self.seal(mir::Terminator::Branch {
                        cond: test,
                        then_block: pass_block,
                        else_block: failure_block,
                    });
                    self.enter(pass_block);
                }
            }
        }
        assert!(
            pass_blocks.next().is_none(),
            "all pattern success blocks are consumed"
        );

        self.lower_statements(&decision.then_body);
        if !self.current_sealed {
            self.seal(mir::Terminator::Goto(merge_block));
        }
        if let Some(else_block) = else_block {
            self.enter(else_block);
            self.lower_statements(&decision.else_body);
            if !self.current_sealed {
                self.seal(mir::Terminator::Goto(merge_block));
            }
        }
        self.enter(merge_block);
    }
}
