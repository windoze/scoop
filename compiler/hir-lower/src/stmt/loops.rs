use super::*;

#[derive(Clone)]
pub(crate) struct ActiveLoop {
    target: hir::LoopId,
    label: Option<String>,
}

impl Lowerer {
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
