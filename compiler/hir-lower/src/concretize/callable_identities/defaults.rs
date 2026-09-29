use std::collections::HashSet;

use super::*;

impl CallableIdentityBuilder<'_> {
    pub(super) fn materialize_default_local_values(
        &mut self,
    ) -> Vec<concrete::DefaultLocalValueScope> {
        let mut scopes = Vec::new();
        let mut seen = HashSet::new();
        for key in &self.concretizer.function_keys {
            let FunctionSource::Local(source) = self.concretizer.function_source(key) else {
                continue;
            };
            let Some(site) = self.lexical_sites.get(&source).cloned() else {
                continue;
            };
            let arguments = self.concretizer.function_key_arguments(key);
            self.collect_default_local_scopes(
                site.root,
                &site.path,
                &arguments,
                &mut seen,
                &mut scopes,
            );
        }
        for pending in &self.concretizer.callable_reference_slots {
            let CallableReferenceSource::Local(source) = pending.source else {
                continue;
            };
            let source = &self.concretizer.source.callable_references[source];
            self.collect_default_local_scopes(
                source.definition_root,
                &source.definition_path,
                &pending.owner_arguments,
                &mut seen,
                &mut scopes,
            );
        }
        scopes
    }

    fn collect_default_local_scopes(
        &mut self,
        root: export::LexicalDefinitionRoot,
        path: &StructuralDefinitionPath,
        arguments: &[concrete::TypeId],
        seen: &mut HashSet<(export::DefaultLocalValueScopeId, CallableMaterialization)>,
        output: &mut Vec<concrete::DefaultLocalValueScope>,
    ) {
        for (id, scope) in self.concretizer.source.default_local_value_scopes.iter() {
            if scope.definition_root != root
                || scope.definition_path.segments().len() >= path.segments().len()
                || !path
                    .segments()
                    .starts_with(scope.definition_path.segments())
            {
                continue;
            }
            let owner =
                self.enclosing_materialization(None, root, &scope.definition_path, arguments);
            if !seen.insert((id, owner)) {
                continue;
            }
            output.push(concrete::DefaultLocalValueScope {
                owner,
                values: scope
                    .values
                    .iter()
                    .map(|value| concrete::DefaultLocalValueDefinition {
                        binding: concrete::BindingId::from_raw(value.binding.into_raw()),
                        selector: value.selector.clone(),
                        definition: value.definition,
                    })
                    .collect(),
            });
        }
    }
}
