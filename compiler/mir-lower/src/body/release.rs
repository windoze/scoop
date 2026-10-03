use super::*;

impl BodyLowerer<'_> {
    pub(crate) fn lower_release_hook(mut self, id: hir::ReleaseHookId) -> smir::Body {
        let hook = &self.module.release_hooks[id];
        let identities = hook
            .body
            .locals
            .iter()
            .map(|(local, _)| {
                self.module
                    .local_value_identities
                    .release_local(id, local)
                    .clone()
            })
            .collect();
        self.allocate_source_locals(&hook.body.locals, identities);
        let statements = self.lower_statements(&hook.body.statements);
        smir::Body {
            locals: self.locals,
            statements,
            coroutine_eh: None,
        }
    }
}
