use super::*;

impl Lowerer {
    /// Reject declarations that differ only in return type after signatures
    /// have been fully resolved.
    pub(crate) fn validate_and_freeze_duplicate_signatures(
        &mut self,
        pending_functions: &[crate::declarations::FunctionHeaderInput<'_>],
        pending_methods: &[crate::declarations::MethodHeaderInput<'_>],
    ) {
        self.validate_and_freeze_duplicate_signatures_with_cancellation(
            pending_functions,
            pending_methods,
            &crate::pipeline::cancellation::uninterrupted,
        )
        .unwrap_or_else(|never| match never {});
    }

    pub(crate) fn validate_and_freeze_duplicate_signatures_with_cancellation<C>(
        &mut self,
        pending_functions: &[crate::declarations::FunctionHeaderInput<'_>],
        pending_methods: &[crate::declarations::MethodHeaderInput<'_>],
        check: &impl Fn() -> Result<(), C>,
    ) -> Result<(), C> {
        check()?;
        let mut rejected = std::collections::HashSet::new();
        let mut ordered_functions = pending_functions.to_vec();
        ordered_functions
            .retain(|&(id, _, _)| self.declaration_surface.unavailable_signature(id).is_none());
        ordered_functions.sort_by_key(|(id, _, _)| self.callable_declaration_order(*id));
        for (index, &(id, decl, file_index)) in ordered_functions.iter().enumerate() {
            check()?;
            let duplicates = ordered_functions[..index]
                .iter()
                .filter_map(|&(other, _, other_file)| {
                    (self
                        .top_level_namespaces
                        .sources_share_namespace(file_index, other_file)
                        && self.functions[other].name == decl.name.text
                        && self.same_parameter_signature(id, other)
                        && (self.functions[id].access.declared != hir::DeclaredVisibility::Private
                            || self.functions[other].access.declared
                                != hir::DeclaredVisibility::Private
                            || file_index == other_file))
                        .then_some(other)
                })
                .collect::<Vec<_>>();
            if !duplicates.is_empty() {
                rejected.insert(id);
                rejected.extend(duplicates);
                self.current_file = file_index;
                self.error(
                    decl.name.span,
                    format!(
                        "function `{}` is already declared with the same signature",
                        decl.name.text
                    ),
                );
            }
        }
        let mut ordered_methods = pending_methods.to_vec();
        ordered_methods
            .retain(|&(id, _, _, _)| self.declaration_surface.unavailable_signature(id).is_none());
        ordered_methods.sort_by_key(|(id, _, _, _)| self.callable_declaration_order(*id));
        for (index, &(id, decl, file_index, owner)) in ordered_methods.iter().enumerate() {
            check()?;
            let duplicates = ordered_methods[..index]
                .iter()
                .filter_map(|&(other, _, _, other_owner)| {
                    (other_owner == owner
                        && self.functions[other].name == self.functions[id].name
                        && self.same_parameter_signature(id, other))
                    .then_some(other)
                })
                .collect::<Vec<_>>();
            if !duplicates.is_empty() {
                rejected.insert(id);
                rejected.extend(duplicates);
                let host = owner.describe(self);
                self.current_file = file_index;
                self.error(
                    decl.name.span,
                    format!(
                        "function `{}` in {host} is already declared with the same signature",
                        decl.name.text
                    ),
                );
            }
        }
        check()?;
        self.declaration_surface.freeze(rejected);
        Ok(())
    }

    /// Persistent source identity, followed by source span, defines declaration
    /// order independently from dense container positions and display paths.
    fn callable_declaration_order(
        &self,
        function: FunctionId,
    ) -> (scoop_identity::SourceIdentity, u32, u32, u32) {
        let file = self.function_files[&function];
        let source = self.visibility_file(file);
        let span = self.functions[function].span;
        (source, span.start, span.end, function.into_raw().into_u32())
    }
}
