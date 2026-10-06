use super::*;

impl CallableIdentityBuilder<'_> {
    pub(super) fn resolve_function(&mut self, index: usize) -> CallableMaterialization {
        if let Some(materialization) = self.materializations[index] {
            return materialization;
        }
        assert!(
            !std::mem::replace(&mut self.visiting[index], true),
            "concrete callable materialization parents are acyclic"
        );
        let key = self.concretizer.function_keys[index].clone();
        let source = match self.concretizer.function_source(&key) {
            FunctionSource::Companion(template, role) => {
                let record = self.concretizer.source.imported_companion_templates[template]
                    .callable(role)
                    .clone();
                let unit = self.concretizer.initialization_map[&InitializationKey {
                    source: initialization::InitializationSource::ImportedCompanion(template),
                    arguments: key.arguments.clone(),
                }];
                let materialization = CallableMaterialization::new(
                    CallableTemplateOwner::Generated(record.id()),
                    CallableMaterializationContext::InitializationApplication(
                        self.concretizer.initialization_units[unit].identity.id(),
                    ),
                );
                self.generated_callable_identities
                    .entry(record.id())
                    .or_insert(record);
                self.visiting[index] = false;
                self.materializations[index] = Some(materialization);
                return materialization;
            }
            FunctionSource::Local(source) => source,
            FunctionSource::Imported(source) => {
                let declaration = self.concretizer.source.imported_generic_templates[source]
                    .declaration
                    .clone();
                let arguments = self.concretizer.function_key_arguments(&key);
                let owner = key.owner;
                let materialization = self.imported_materialization(declaration, owner, &arguments);
                self.visiting[index] = false;
                self.materializations[index] = Some(materialization);
                return materialization;
            }
        };
        let identity = self.concretizer.source.function_identities[source].clone();
        let materialization = match identity {
            export::HirFunctionIdentity::Source(identity) => {
                let template = match identity {
                    export::HirSourceFunctionIdentity::Plain(record) => {
                        SourceTemplate::Function(record.id())
                    }
                    export::HirSourceFunctionIdentity::Generic(record) => {
                        SourceTemplate::GenericFunction(record.id())
                    }
                };
                if let Some(site) = self.lexical_sites.get(&source).cloned() {
                    self.local_source_materialization(&key, template, &site)
                } else {
                    self.declaration_materialization(&key, template)
                }
            }
            export::HirFunctionIdentity::PropertyAccessor(accessor) => {
                let accessor = match accessor {
                    export::HirPropertyAccessorFunction::Getter(getter) => self
                        .concretizer
                        .source
                        .property_accessor_identities
                        .get_getter(getter),
                    export::HirPropertyAccessorFunction::Setter(setter) => self
                        .concretizer
                        .source
                        .property_accessor_identities
                        .get_setter(setter),
                }
                .expect("the validated accessor identity relation is total");
                let extension = self.concretizer.source.property_identities[accessor.property()]
                    .extension_id()
                    .is_some();
                self.declaration_materialization(
                    &key,
                    SourceTemplate::Accessor {
                        id: accessor.id(),
                        extension,
                    },
                )
            }
            export::HirFunctionIdentity::LexicalGenerated(record) => {
                let site = self
                    .lexical_sites
                    .get(&source)
                    .cloned()
                    .expect("a lexical generated callable retains its source site");
                let arguments = self.concretizer.function_key_arguments(&key);
                assert_eq!(arguments.len(), site.owner_type_parameter_count);
                CallableMaterialization::new(
                    CallableTemplateOwner::Generated(record.id()),
                    self.lexical_context(source, &site, &arguments),
                )
            }
            export::HirFunctionIdentity::Initialization { record, unit, .. } => {
                let arguments = self.concretizer.function_key_arguments(&key);
                let context = if arguments.is_empty() {
                    CallableMaterializationContext::NoSubstitution
                } else {
                    let unit = self.concretizer.initialization_map[&InitializationKey {
                        source: initialization::InitializationSource::Defined(unit),
                        arguments,
                    }];
                    CallableMaterializationContext::InitializationApplication(
                        self.concretizer.initialization_units[unit].identity.id(),
                    )
                };
                CallableMaterialization::new(CallableTemplateOwner::Generated(record.id()), context)
            }
            export::HirFunctionIdentity::DerivedEquality(_) => {
                let exact_owner = self.exact_method_owner(
                    key.owner
                        .expect("a type-owned method always has a complete owner"),
                );
                let role = GeneratedCallableKey::DerivedEquality { exact_owner };
                let record = CborIdentityRecord::from_key(role)
                    .expect("a type-owned method has its complete exact owner");
                let id = record.id();
                self.generated_callable_identities
                    .entry(id)
                    .or_insert(record);
                CallableMaterialization::new(
                    CallableTemplateOwner::Generated(id),
                    CallableMaterializationContext::NoSubstitution,
                )
            }
        };
        self.visiting[index] = false;
        self.materializations[index] = Some(materialization);
        materialization
    }
}
