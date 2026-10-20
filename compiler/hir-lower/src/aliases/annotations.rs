use super::*;

impl Lowerer {
    pub(crate) fn source_alias_type_use(&self, id: SourceTypeAliasId) -> (usize, ast::Span) {
        let alias = &self.source_type_aliases[id];
        (alias.file, alias.target.span)
    }

    pub(crate) fn annotate_type_aliases(&mut self) {
        self.annotate_aliases_where(|alias| alias.scope.is_none());
    }

    pub(crate) fn annotate_local_owner_aliases(&mut self, owners: &[crate::Owner]) {
        self.annotate_aliases_where(|alias| {
            alias.scope.is_some()
                && !alias.lexical
                && alias.owner.is_some_and(|owner| owners.contains(&owner))
        });
    }

    fn annotate_aliases_where(&mut self, selected: impl Fn(&SourceTypeAlias) -> bool) {
        let aliases = self
            .source_type_aliases
            .iter()
            .filter_map(|(_, alias)| {
                if !selected(alias) {
                    return None;
                }
                let TypeAliasResolution::Published { declaration, .. } = alias.resolution else {
                    return None;
                };
                Some((declaration, alias.clone()))
            })
            .collect::<Vec<_>>();
        for (declaration, alias) in aliases {
            self.current_file = alias.file;
            self.current_owner = alias.owner;
            self.type_params_in_scope = alias.parameters;
            self.document_alias(declaration, &alias.syntax_parameters);
            self.bind_source_annotations(
                hir::SourceAnnotationTarget::TypeAlias(declaration),
                &alias.annotations,
            );
        }
    }
}
