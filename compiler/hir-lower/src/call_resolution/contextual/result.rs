use super::*;

impl Lowerer {
    pub(super) fn infer_argument_result(
        &mut self,
        result: DeclarationResult,
        values: &[Option<hir::Expr>],
    ) -> Result<hir::TypeId, Box<ArgumentInferenceFailure>> {
        let before = self.diagnostics.len();
        self.resolve_declaration_result(result).ok_or_else(|| {
            let span = self
                .diagnostics
                .get(before)
                .and_then(|diagnostic| diagnostic.span)
                .unwrap_or_else(|| match result {
                    DeclarationResult::Function(function) => self.functions[function].span,
                    DeclarationResult::Known(_) => unreachable!("a known result always resolves"),
                });
            let reason = self.diagnostics[before..]
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Box::new(ArgumentInferenceFailure {
                arguments: values.to_vec(),
                kind: ArgumentInferenceFailureKind::Declaration {
                    span,
                    reason: if reason.is_empty() {
                        "declaration result type could not be inferred".into()
                    } else {
                        reason
                    },
                },
            })
        })
    }
}
