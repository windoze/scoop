use super::*;
use crate::editing::EditingCancelled;

impl Lowerer {
    pub(crate) fn capture_editing_alias_documentation(
        &mut self,
        check: &impl Fn() -> Result<(), EditingCancelled>,
    ) -> Result<(), EditingCancelled> {
        let aliases = self
            .source_type_aliases
            .iter()
            .filter_map(|(_, alias)| {
                let TypeAliasResolution::Published { declaration, .. } = alias.resolution else {
                    return None;
                };
                alias.scope.is_none().then(|| {
                    (
                        declaration,
                        alias.file,
                        alias.owner,
                        alias.parameters.clone(),
                        alias.syntax_parameters.clone(),
                    )
                })
            })
            .collect::<Vec<_>>();
        let previous = (
            self.current_file,
            self.current_owner,
            self.type_params_in_scope.clone(),
        );
        let result = (|| {
            for (declaration, file, owner, parameters, syntax) in aliases {
                check()?;
                self.current_file = file;
                self.current_owner = owner;
                self.type_params_in_scope = parameters;
                self.document_alias(declaration, &syntax);
            }
            Ok(())
        })();
        (
            self.current_file,
            self.current_owner,
            self.type_params_in_scope,
        ) = previous;
        result
    }
}
