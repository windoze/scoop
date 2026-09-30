use std::collections::HashSet;

use super::*;

impl CallableIdentityBuilder<'_> {
    pub(super) fn materialize_default_local_values(
        &mut self,
    ) -> Vec<concrete::DefaultLocalValueScope> {
        let mut scopes = Vec::new();
        let mut seen = HashSet::new();
        for key in &self.concretizer.function_keys {
            let source = match self.concretizer.function_source(key) {
                FunctionSource::Local(source) => source,
                FunctionSource::Imported(source) => {
                    let origin =
                        &self.concretizer.source.imported_generic_templates[source].declaration;
                    let (parent, arguments) = match origin {
                        export::ImportedCallableTemplateOrigin::Closure { parent, .. } => {
                            (*parent, &key.arguments[..])
                        }
                        export::ImportedCallableTemplateOrigin::Local {
                            parent,
                            descriptor,
                            ..
                        } => (
                            *parent,
                            &key.arguments[..descriptor.owner_type_parameter_count() as usize],
                        ),
                        _ => continue,
                    };
                    self.collect_loaded_default_scopes(parent, arguments, &mut seen, &mut scopes);
                    continue;
                }
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
            let source = &self.concretizer.source.callable_references[pending.source];
            match source.definition_root {
                export::CallableReferenceRoot::Persistent(parent) => {
                    self.collect_loaded_default_scopes(
                        parent.template(),
                        &pending.owner_arguments,
                        &mut seen,
                        &mut scopes,
                    );
                }
                export::CallableReferenceRoot::Source(root) => self.collect_default_local_scopes(
                    root,
                    &source.definition_path,
                    &pending.owner_arguments,
                    &mut seen,
                    &mut scopes,
                ),
            }
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
        let definition_root = self.source_default_root(root);
        for (id, scope) in self.concretizer.source.default_local_value_scopes.iter() {
            if scope.definition_root != definition_root
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

    fn source_default_root(&self, root: export::LexicalDefinitionRoot) -> CallableTemplateOwner {
        match root {
            export::LexicalDefinitionRoot::Function(function) => self
                .concretizer
                .function_key(FunctionSource::Local(function), None, Vec::new())
                .template_owner()
                .expect("a default has a source body root"),
            export::LexicalDefinitionRoot::ClassConstructor(constructor) => {
                CallableTemplateOwner::Constructor(
                    self.concretizer.source.constructor_identities[constructor]
                        .source_record()
                        .expect("source defaults have source constructors")
                        .id(),
                )
            }
            export::LexicalDefinitionRoot::StructConstructor(constructor) => {
                CallableTemplateOwner::Constructor(
                    self.concretizer.source.constructor_identities[constructor].id(),
                )
            }
            export::LexicalDefinitionRoot::VariantConstructor(variant) => {
                CallableTemplateOwner::VariantConstructor(
                    self.concretizer.source.enum_member_identities[variant].id(),
                )
            }
        }
    }

    fn collect_loaded_default_scopes(
        &mut self,
        parent: CallableTemplateOwner,
        arguments: &[concrete::TypeId],
        seen: &mut HashSet<(export::DefaultLocalValueScopeId, CallableMaterialization)>,
        output: &mut Vec<concrete::DefaultLocalValueScope>,
    ) {
        let scopes = self
            .concretizer
            .source
            .default_local_value_scopes
            .iter()
            .filter(|(_, scope)| scope.definition_root == parent)
            .collect::<Vec<_>>();
        if scopes.is_empty() {
            return;
        }
        let owner = self.imported_parent_materialization(parent, arguments);
        for (id, scope) in scopes {
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
