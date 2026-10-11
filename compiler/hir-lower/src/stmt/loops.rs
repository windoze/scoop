use super::*;

#[derive(Clone)]
pub(crate) struct ActiveLoop {
    target: hir::LoopId,
    label: Option<String>,
}

impl Lowerer {
    pub(super) fn lower_loop(&mut self, source: &ast::While) -> Option<hir::StatementKind> {
        let target = self.fresh_loop();
        self.push_loop(target, source.label.as_ref());
        let mut condition_setup = Vec::new();
        let (body, cond) = match source.loop_kind {
            hir::LoopKind::While => {
                let cond = self.lower_condition(&source.cond, "while", &mut condition_setup);
                if cond.is_none() {
                    self.pop_loop(target);
                    return None;
                }
                let body = self.lower_block(&source.body);
                (body, cond)
            }
            hir::LoopKind::DoWhile => {
                self.push_scope();
                let mut body = Vec::new();
                for statement in &source.body.statements {
                    self.lower_statement(statement, &mut body);
                }
                let previous = self.uninitialized_condition_bindings.clone();
                let skipped = self.bindings_skipped_by_continue(&body, target);
                self.uninitialized_condition_bindings.extend(skipped);
                let cond = self.lower_condition(&source.cond, "do-while", &mut condition_setup);
                self.uninitialized_condition_bindings = previous;
                self.pop_scope();
                (body, cond)
            }
        };
        self.pop_loop(target);
        Some(hir::StatementKind::While {
            loop_kind: source.loop_kind,
            target,
            condition_setup,
            cond: cond?,
            body,
        })
    }

    pub(crate) fn loop_label_is_active(&self, name: &str) -> bool {
        self.loop_targets
            .iter()
            .rev()
            .any(|target| target.label.as_deref() == Some(name))
    }

    pub(crate) fn push_loop(&mut self, target: hir::LoopId, label: Option<&ast::Ident>) {
        self.loop_targets.push(ActiveLoop {
            target,
            label: label.map(|label| label.text.clone()),
        });
    }

    pub(crate) fn pop_loop(&mut self, target: hir::LoopId) {
        assert_eq!(
            self.loop_targets.pop().map(|loop_| loop_.target),
            Some(target)
        );
    }

    pub(crate) fn lower_loop_jump(
        &mut self,
        kind: ast::LoopJumpKind,
        label: Option<&ast::Ident>,
        span: Span,
    ) -> Option<hir::StatementKind> {
        let target = self
            .loop_targets
            .iter()
            .rev()
            .find(|loop_| {
                label.is_none_or(|label| loop_.label.as_deref() == Some(label.text.as_str()))
            })
            .map(|loop_| loop_.target);
        let Some(target) = target else {
            let keyword = kind.keyword();
            match label {
                Some(label) => self.error(
                    label.span,
                    format!(
                        "unknown loop label `{}` in this callable for `{keyword}`",
                        label.text
                    ),
                ),
                None => self.error(span, format!("`{keyword}` is only allowed inside a loop")),
            }
            return None;
        };
        Some(match kind {
            ast::LoopJumpKind::Break => hir::StatementKind::Break { target },
            ast::LoopJumpKind::Continue => hir::StatementKind::Continue { target },
        })
    }
}
