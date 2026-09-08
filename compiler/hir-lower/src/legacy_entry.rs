use super::*;

impl Lowerer {
    /// Select the temporary M22-compatible executable entry point.
    ///
    /// Qualification is deliberately complete before cardinality is checked:
    /// declarations which merely happen to be named `main` cannot make an
    /// otherwise unique executable entry ambiguous. The namespace query is
    /// restricted to ordinary functions in the current user unit; core and
    /// extension declarations never enter this candidate set.
    pub(crate) fn select_legacy_entry(
        &mut self,
        files: &[ast::SourceFile],
        primary_user_file: usize,
    ) -> Result<FunctionId, ()> {
        self.current_file = primary_user_file;
        let named = self
            .top_level_namespaces
            .current_unit_functions_named("main");
        let candidates = named
            .iter()
            .copied()
            .filter(|id| self.is_legacy_entry_candidate(*id))
            .collect::<Vec<_>>();

        match candidates.as_slice() {
            [entry] => Ok(*entry),
            [] if !self.diagnostics.is_empty() => {
                // Recovery can assign fallback types to a malformed `main`.
                // Its source error is authoritative; an entry-shape error
                // derived from that fallback would only be misleading.
                Err(())
            }
            [] => {
                if !self.diagnose_invalid_legacy_mains(&named) {
                    self.current_file = primary_user_file;
                    self.error(
                        files[primary_user_file].span,
                        "missing entry point: declare `fun main()`".to_string(),
                    );
                }
                Err(())
            }
            candidates if self.candidates_are_existing_duplicates(candidates) => {
                // Duplicate-signature checking has already rejected this HIR.
                // Preserve its focused diagnostic instead of adding a
                // secondary executable-cardinality error.
                Ok(candidates[0])
            }
            _ => {
                self.error(
                    files[primary_user_file].span,
                    "multiple entry points: declare exactly one `fun main()`".to_string(),
                );
                Err(())
            }
        }
    }

    fn is_legacy_entry_candidate(&self, id: FunctionId) -> bool {
        let function = &self.functions[id];
        self.signatures.get(&id).is_some_and(|signature| {
            function.method.is_none()
                && matches!(&function.kind, FunctionKind::User(_))
                && matches!(&function.genericity, hir::FunctionGenericity::Plain)
                && !function.is_suspend
                && signature.params.is_empty()
                && signature.return_ty == self.unit
        })
    }

    /// Retain the established focused diagnostics when there is exactly no
    /// fully valid entry. Invalid declarations stay silent when a valid entry
    /// exists, because they are ordinary overloads rather than entry points.
    fn diagnose_invalid_legacy_mains(&mut self, named: &[FunctionId]) -> bool {
        let mut diagnosed = false;
        for id in named.iter().copied() {
            let Some(signature) = self.signatures.get(&id) else {
                continue;
            };
            if self.functions[id].method.is_some() || !signature.params.is_empty() {
                continue;
            }

            self.current_file = self.function_files[&id];
            let span = self.functions[id].span;
            let is_generic = !matches!(
                &self.functions[id].genericity,
                hir::FunctionGenericity::Plain
            );
            let is_suspend = self.functions[id].is_suspend;
            let is_user = matches!(&self.functions[id].kind, FunctionKind::User(_));
            let returns_unit = signature.return_ty == self.unit;

            if is_generic {
                self.error(span, "`main` must not be generic".to_string());
                diagnosed = true;
            }
            if is_suspend {
                self.error(span, "`main` must not be suspend".to_string());
                diagnosed = true;
            }
            if !is_user {
                self.error(span, "`main` must be a Scoop-defined function".to_string());
                diagnosed = true;
            }
            if !returns_unit {
                self.error(span, "`main` must return `Unit`".to_string());
                diagnosed = true;
            }
        }
        diagnosed
    }

    fn candidates_are_existing_duplicates(&self, candidates: &[FunctionId]) -> bool {
        candidates.iter().enumerate().all(|(index, id)| {
            candidates[..index].iter().all(|other| {
                let file = self.function_files[id];
                let other_file = self.function_files[other];
                self.top_level_namespaces
                    .sources_share_namespace(file, other_file)
                    && (self.functions[*id].access.declared != hir::DeclaredVisibility::Private
                        || self.functions[*other].access.declared
                            != hir::DeclaredVisibility::Private
                        || file == other_file)
            })
        })
    }
}
