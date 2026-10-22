use super::*;

impl Lowerer {
    pub(super) fn commit_named_function_partition(
        &mut self,
        applicable: Vec<NamedApplicable>,
        targets: &[NamedCallBinding],
        kind: ImportLookupLayer,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Result<Option<hir::Expr>, ()> {
        let (probes, mut commits): (Vec<_>, Vec<_>) = applicable
            .into_iter()
            .map(|applicable| (applicable.probe, applicable.commit))
            .unzip();
        let diagnostics_before = self.diagnostics.len();
        let Some(winner) = self.select_named_function_like(
            &call.callee.text,
            kind.call_name(),
            &probes,
            &call.args,
            call.span,
        ) else {
            let variants = targets
                .iter()
                .filter_map(|binding| match binding.target {
                    NamedCallTarget::Value(ValueTarget::Variant(target)) => Some(target),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if kind == ImportLookupLayer::CorePrelude && variants.len() == targets.len() {
                self.diagnostics.truncate(diagnostics_before);
                self.ambiguous_prelude_variant(&call.callee, &variants);
            }
            return Err(());
        };
        let mut probes = probes;
        let probe = probes.swap_remove(winner);
        let commit = commits.swap_remove(winner);
        let expression = match (probe, commit) {
            (NamedFunctionLikeProbe::Callable(probe), NamedFunctionCommit::TopLevel) => {
                let resolved = self.commit_named_callable(*probe, sink).ok_or(())?;
                self.finish_resolved_top_level_function_call(call, resolved, sink)
            }
            (NamedFunctionLikeProbe::Callable(probe), NamedFunctionCommit::Member) => {
                let resolved = self.commit_named_callable(*probe, sink).ok_or(())?;
                self.observe_callable_reference(
                    call.callee.span,
                    hir::SemanticCallable::Current(resolved.function()),
                );
                self.finish_resolved_method_call(resolved, call.span, sink)
            }
            (
                NamedFunctionLikeProbe::ImportedDependency(probe),
                NamedFunctionCommit::ImportedDependency,
            ) => {
                let target = probe.semantic_target();
                let expression = self.commit_imported_dependency_callable(*probe, sink);
                if expression.is_some()
                    && let Some(target) = target
                {
                    self.observe_callable_reference(call.callee.span, target);
                }
                return Ok(expression);
            }
            (NamedFunctionLikeProbe::Nominal(probe), NamedFunctionCommit::Nominal) => {
                let resolved = self
                    .commit_named_nominal(*probe, call.span, sink)
                    .ok_or(())?;
                self.finish_named_nominal(resolved, call.span)
            }
            (NamedFunctionLikeProbe::IntrinsicStruct(_), NamedFunctionCommit::Intrinsic(layer)) => {
                if layer.state.diagnostics[diagnostics_before..]
                    .iter()
                    .any(|diagnostic| diagnostic.severity == ast::DiagnosticSeverity::Error)
                {
                    self.commit_layer_diagnostics(*layer.state);
                    None
                } else {
                    Some(self.commit_expr_layer(layer, sink))
                }
            }
            _ => unreachable!("each typed probe retains its matching commit protocol"),
        };
        expression.map(Some).ok_or(())
    }
}
