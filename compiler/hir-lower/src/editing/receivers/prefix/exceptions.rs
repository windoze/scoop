use super::*;

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    pub(super) fn try_(
        &mut self,
        lowerer: &mut Lowerer,
        try_: &ast::RecoveredTry,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        let incoming = lowerer.smart_casts.clone();
        lowerer.with_smart_casts(incoming.clone(), |lowerer| {
            let body = self.try_block(lowerer, &try_.body, available)?;
            let catch_facts = match &body {
                Some(body) => lowerer.catch_smart_cast_facts(incoming.clone(), body, None),
                None => lowerer.facts_without_mutable_bindings(incoming.clone()),
            };
            let mut precise_finally = try_.finally_body.is_some()
                && body.is_some()
                && try_.catches.iter().all(|catch| {
                    matches!(
                        catch,
                        ast::RecoveredCatch::Clause {
                            body: ast::RecoveredTryBlock::Complete(_),
                            ..
                        }
                    )
                });
            let mut covered = Vec::new();
            let mut checked_catches = Vec::new();
            for catch in &try_.catches {
                check_cancelled(self.is_cancelled)?;
                let ast::RecoveredCatch::Clause {
                    name,
                    ty,
                    body,
                    span,
                } = catch
                else {
                    return Ok(());
                };
                let before = lowerer.diagnostics.len();
                let ty = available
                    .then(|| {
                        lowerer
                            .resolve_catch_type(ty, *span, covered.iter().copied())
                            .ok()
                            .flatten()
                    })
                    .flatten()
                    .filter(|_| lowerer.diagnostics.len() == before);
                let Some(ty) = ty else {
                    precise_finally = false;
                    if let ast::RecoveredTryBlock::Incomplete(id) = body {
                        self.block(lowerer, *id, false)?;
                    }
                    continue;
                };
                covered.push(ty);
                if !precise_finally && matches!(body, ast::RecoveredTryBlock::Complete(_)) {
                    continue;
                }
                let local = lowerer.alloc_declared_local(name.text.clone(), ty, false, name.span);
                let checked = lowerer.with_catch_scope(name, local, |lowerer| {
                    lowerer.with_smart_casts(catch_facts.clone(), |lowerer| {
                        self.try_block(lowerer, body, true)
                    })
                })?;
                if let Some(checked) = checked {
                    checked_catches.push(checked);
                } else {
                    precise_finally = false;
                }
            }
            if let Some(finally) = try_.finally_body {
                check_cancelled(self.is_cancelled)?;
                let facts = if precise_finally {
                    lowerer.finally_smart_cast_facts(
                        incoming.clone(),
                        body.as_ref()
                            .expect("precise finally facts have a complete try body"),
                        checked_catches.iter().map(Vec::as_slice),
                    )
                } else {
                    lowerer.facts_without_mutable_bindings(incoming)
                };
                lowerer
                    .with_smart_casts(facts, |lowerer| self.block(lowerer, finally, available))?;
            }
            Ok(())
        })
    }

    fn try_block(
        &mut self,
        lowerer: &mut Lowerer,
        body: &ast::RecoveredTryBlock,
        available: bool,
    ) -> Result<Option<Vec<hir::Statement>>, EditingCancelled> {
        match body {
            ast::RecoveredTryBlock::Complete(body) => {
                lowerer.push_scope();
                let result = self.complete_statements(lowerer, &body.statements, available);
                lowerer.pop_scope();
                result
            }
            ast::RecoveredTryBlock::Incomplete(id) => {
                self.block(lowerer, *id, available)?;
                Ok(None)
            }
        }
    }
}
