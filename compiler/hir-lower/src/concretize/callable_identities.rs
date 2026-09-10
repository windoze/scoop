use std::collections::HashMap;

use scoop_identity::{
    CallableApplicationKey, CallableInstantiationOwner, CallableMaterialization,
    CallableMaterializationContext, CallableTemplateOwner, CborIdentityRecord,
    GeneratedCallableKey, NonEmptyVec, PersistentCallableApplicationId, StructuralDefinitionPath,
};

use super::*;

mod lexical;

#[derive(Clone, Eq, PartialEq)]
struct LexicalSite {
    root: export::LexicalDefinitionRoot,
    path: StructuralDefinitionPath,
    owner_type_parameter_count: usize,
}

#[derive(Clone, Copy)]
enum SourceTemplate {
    Function(scoop_identity::PersistentFunctionId),
    GenericFunction(scoop_identity::PersistentGenericFunctionId),
    Accessor {
        id: scoop_identity::PersistentPropertyAccessorId,
        extension: bool,
    },
}

impl Concretizer<'_> {
    pub(super) fn build_callable_identities(
        &self,
        exact_types: &concrete::ExactTypeIdentities,
    ) -> (
        concrete::CallableApplicationIdentities,
        Vec<CallableMaterialization>,
        Vec<concrete::FunctionEmission>,
        Vec<CallableMaterialization>,
        Vec<CallableMaterialization>,
    ) {
        CallableIdentityBuilder::new(self, exact_types).build()
    }
}

struct CallableIdentityBuilder<'a> {
    concretizer: &'a Concretizer<'a>,
    exact_types: &'a concrete::ExactTypeIdentities,
    lexical_sites: HashMap<export::FunctionId, LexicalSite>,
    materializations: Vec<Option<CallableMaterialization>>,
    class_constructor_materializations: Vec<Option<CallableMaterialization>>,
    struct_constructor_materializations: Vec<Option<CallableMaterialization>>,
    visiting: Vec<bool>,
    applications: Vec<concrete::CallableApplicationRecord>,
    application_by_key: HashMap<CallableApplicationKey, PersistentCallableApplicationId>,
}

impl<'a> CallableIdentityBuilder<'a> {
    fn new(
        concretizer: &'a Concretizer<'a>,
        exact_types: &'a concrete::ExactTypeIdentities,
    ) -> Self {
        let mut lexical_sites = HashMap::new();
        for (_, declaration) in concretizer.source.local_functions.iter() {
            insert_lexical_site(
                &mut lexical_sites,
                declaration.function,
                LexicalSite {
                    root: declaration.definition_root,
                    path: declaration.definition_path.clone(),
                    owner_type_parameter_count: declaration.owner_type_param_count,
                },
            );
        }
        for (_, declaration) in concretizer.source.lambdas.iter() {
            insert_lexical_site(
                &mut lexical_sites,
                declaration.function,
                LexicalSite {
                    root: declaration.definition_root,
                    path: declaration.definition_path.clone(),
                    owner_type_parameter_count: declaration.owner_type_param_count,
                },
            );
        }
        for (_, declaration) in concretizer.source.anonymous_functions.iter() {
            insert_lexical_site(
                &mut lexical_sites,
                declaration.function,
                LexicalSite {
                    root: declaration.definition_root,
                    path: declaration.definition_path.clone(),
                    owner_type_parameter_count: declaration.owner_type_param_count,
                },
            );
        }
        Self {
            concretizer,
            exact_types,
            lexical_sites,
            materializations: vec![None; concretizer.function_keys.len()],
            class_constructor_materializations: vec![
                None;
                concretizer.class_constructor_keys.len()
            ],
            struct_constructor_materializations: vec![
                None;
                concretizer.struct_constructor_keys.len()
            ],
            visiting: vec![false; concretizer.function_keys.len()],
            applications: Vec::new(),
            application_by_key: HashMap::new(),
        }
    }

    fn build(
        mut self,
    ) -> (
        concrete::CallableApplicationIdentities,
        Vec<CallableMaterialization>,
        Vec<concrete::FunctionEmission>,
        Vec<CallableMaterialization>,
        Vec<CallableMaterialization>,
    ) {
        for index in 0..self.concretizer.function_keys.len() {
            self.resolve_function(index);
        }
        for index in 0..self.concretizer.class_constructor_keys.len() {
            self.resolve_class_constructor(index);
        }
        for index in 0..self.concretizer.struct_constructor_keys.len() {
            self.resolve_struct_constructor(index);
        }
        let emissions = self
            .concretizer
            .function_keys
            .iter()
            .map(|key| self.emission(key))
            .collect();
        let materializations = std::mem::take(&mut self.materializations)
            .into_iter()
            .enumerate()
            .map(|(index, materialization)| {
                materialization
                    .unwrap_or_else(|| panic!("missing callable materialization at index {index}"))
            })
            .collect::<Vec<_>>();
        let class_constructor_materializations = complete_materializations(
            &mut self.class_constructor_materializations,
            "class constructor",
        );
        let struct_constructor_materializations = complete_materializations(
            &mut self.struct_constructor_materializations,
            "struct constructor",
        );
        let applications = concrete::CallableApplicationIdentities::checked(self.applications)
            .expect("validated concrete callable applications form one complete acyclic table");
        (
            applications,
            materializations,
            emissions,
            class_constructor_materializations,
            struct_constructor_materializations,
        )
    }

    fn resolve_function(&mut self, index: usize) -> CallableMaterialization {
        if let Some(materialization) = self.materializations[index] {
            return materialization;
        }
        assert!(
            !std::mem::replace(&mut self.visiting[index], true),
            "concrete callable materialization parents are acyclic"
        );
        let key = self.concretizer.function_keys[index].clone();
        let source = key.source();
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
            export::HirFunctionIdentity::Initialization { record, .. } => {
                assert!(self.concretizer.function_key_arguments(&key).is_empty());
                CallableMaterialization::new(
                    CallableTemplateOwner::Generated(record.id()),
                    CallableMaterializationContext::NoSubstitution,
                )
            }
            export::HirFunctionIdentity::DerivedEquality(applications) => {
                let exact_owner = self.exact_method_owner(match key {
                    FunctionKey::Method { owner, .. } => owner,
                    FunctionKey::Free { .. } => {
                        panic!("derived equality is always an exact-owner method")
                    }
                });
                let record = applications
                    .iter()
                    .map(export::HirDerivedEqualityFunctionIdentity::record)
                    .find(|record| {
                        matches!(
                            record.key(),
                            GeneratedCallableKey::DerivedEquality { exact_owner: candidate }
                                if *candidate == exact_owner
                        )
                    })
                    .expect("a concrete derived equality retains its exact generated template");
                CallableMaterialization::new(
                    CallableTemplateOwner::Generated(record.id()),
                    CallableMaterializationContext::NoSubstitution,
                )
            }
        };
        self.visiting[index] = false;
        self.materializations[index] = Some(materialization);
        materialization
    }

    fn resolve_class_constructor(&mut self, index: usize) -> CallableMaterialization {
        if let Some(materialization) = self.class_constructor_materializations[index] {
            return materialization;
        }
        let (source, class) = self.concretizer.class_constructor_keys[index];
        let arguments = self.concretizer.classes[class].type_arguments.clone();
        let exact_owner = self.exact_types[self.concretizer.class_type[&class]].id();
        let identity = &self.concretizer.source.constructor_identities[source];
        let (template, origin) = match identity {
            export::HirClassConstructorIdentity::Source(record) => {
                (CallableTemplateOwner::Constructor(record.id()), record.id())
            }
            export::HirClassConstructorIdentity::ZeroArgumentAdapter { source, record } => {
                let origin = self.concretizer.source.constructor_identities[*source]
                    .source_record()
                    .expect("a zero-argument adapter references a source constructor")
                    .id();
                (CallableTemplateOwner::Generated(record.id()), origin)
            }
        };
        let materialization = CallableMaterialization::new(
            template,
            self.constructor_application_context(origin, exact_owner, &arguments),
        );
        self.class_constructor_materializations[index] = Some(materialization);
        materialization
    }

    fn resolve_struct_constructor(&mut self, index: usize) -> CallableMaterialization {
        if let Some(materialization) = self.struct_constructor_materializations[index] {
            return materialization;
        }
        let (source, structure) = self.concretizer.struct_constructor_keys[index];
        let arguments = self.concretizer.structs[structure].type_arguments.clone();
        let exact_owner = self.exact_types[self.concretizer.struct_type[&structure]].id();
        let origin = self.concretizer.source.constructor_identities[source].id();
        let materialization = CallableMaterialization::new(
            CallableTemplateOwner::Constructor(origin),
            self.constructor_application_context(origin, exact_owner, &arguments),
        );
        self.struct_constructor_materializations[index] = Some(materialization);
        materialization
    }

    fn declaration_materialization(
        &mut self,
        key: &FunctionKey,
        template: SourceTemplate,
    ) -> CallableMaterialization {
        match key {
            FunctionKey::Free { arguments, .. } => self.source_materialization(
                template,
                CallableInstantiationOwner::NoOwner,
                arguments,
            ),
            FunctionKey::Method {
                owner,
                specialization,
                ..
            } => {
                let owner_arguments = self.concretizer.concrete_method_owner_arguments(*owner);
                let instantiation_owner = if owner_arguments.is_empty() {
                    CallableInstantiationOwner::NoOwner
                } else {
                    CallableInstantiationOwner::ExactNominalOwner(self.exact_method_owner(*owner))
                };
                let callable_arguments = match specialization {
                    MethodRequest::Plain => Vec::new(),
                    MethodRequest::Generic {
                        method_arguments, ..
                    } => method_arguments.iter().copied().collect(),
                };
                self.source_materialization(template, instantiation_owner, &callable_arguments)
            }
        }
    }

    fn local_source_materialization(
        &mut self,
        key: &FunctionKey,
        template: SourceTemplate,
        site: &LexicalSite,
    ) -> CallableMaterialization {
        let FunctionKey::Free { source, arguments } = key else {
            panic!("a block-local source callable is not a nominal method")
        };
        assert!(site.owner_type_parameter_count <= arguments.len());
        let inherited = &arguments[..site.owner_type_parameter_count];
        let own = &arguments[site.owner_type_parameter_count..];
        let owner = match self.lexical_context(*source, site, inherited) {
            CallableMaterializationContext::NoSubstitution => CallableInstantiationOwner::NoOwner,
            CallableMaterializationContext::Application(application) => {
                CallableInstantiationOwner::EnclosingCallableApplication(application)
            }
            CallableMaterializationContext::InitializationApplication(unit) => {
                CallableInstantiationOwner::EnclosingInitializationApplication(unit)
            }
        };
        self.source_materialization(template, owner, own)
    }

    fn source_materialization(
        &mut self,
        template: SourceTemplate,
        owner: CallableInstantiationOwner,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let template_owner = match template {
            SourceTemplate::Function(id) => CallableTemplateOwner::Function(id),
            SourceTemplate::GenericFunction(id) => CallableTemplateOwner::GenericFunction(id),
            SourceTemplate::Accessor { id, .. } => CallableTemplateOwner::Accessor(id),
        };
        if owner == CallableInstantiationOwner::NoOwner && arguments.is_empty() {
            return CallableMaterialization::new(
                template_owner,
                CallableMaterializationContext::NoSubstitution,
            );
        }

        let exact_arguments = || {
            NonEmptyVec::new(
                arguments
                    .iter()
                    .map(|argument| self.exact_types[*argument].id())
                    .collect(),
            )
            .expect("a callable argument group is non-empty")
        };
        let key = match template {
            SourceTemplate::Function(id) => {
                assert!(arguments.is_empty());
                CallableApplicationKey::for_function(id, owner)
            }
            SourceTemplate::GenericFunction(id) => {
                CallableApplicationKey::for_generic_function(id, owner, exact_arguments())
            }
            SourceTemplate::Accessor {
                id,
                extension: true,
            } => {
                CallableApplicationKey::for_generic_extension_accessor(id, owner, exact_arguments())
            }
            SourceTemplate::Accessor {
                id,
                extension: false,
            } => {
                assert!(arguments.is_empty());
                CallableApplicationKey::for_accessor(id, owner)
            }
        };
        let application = self.record_application(key);
        CallableMaterialization::new(
            template_owner,
            CallableMaterializationContext::Application(application),
        )
    }

    fn record_application(
        &mut self,
        key: CallableApplicationKey,
    ) -> PersistentCallableApplicationId {
        if let Some(application) = self.application_by_key.get(&key) {
            return *application;
        }
        let record = CborIdentityRecord::from_key(key.clone())
            .expect("validated callable application keys are hashable");
        let application = record.id();
        self.application_by_key.insert(key, application);
        self.applications.push(record);
        application
    }

    fn emission(&self, key: &FunctionKey) -> concrete::FunctionEmission {
        let arguments = self.concretizer.function_key_arguments(key);
        if arguments.is_empty() {
            return concrete::FunctionEmission::Direct;
        }
        let arguments = NonEmptyVec::new(arguments).expect("the argument group is non-empty");
        let source = key.source();
        concrete::FunctionEmission::Materialized {
            arguments,
            symbol: self.concretizer.instance_symbol(
                &self.concretizer.source.functions[source].link_stem,
                source.into_raw().into_u32(),
            ),
        }
    }
}

fn insert_lexical_site(
    sites: &mut HashMap<export::FunctionId, LexicalSite>,
    function: export::FunctionId,
    site: LexicalSite,
) {
    if let Some(existing) = sites.insert(function, site.clone()) {
        assert!(
            existing == site,
            "one lexical callable function has one stable definition site"
        );
    }
}

fn complete_materializations(
    materializations: &mut Vec<Option<CallableMaterialization>>,
    description: &str,
) -> Vec<CallableMaterialization> {
    std::mem::take(materializations)
        .into_iter()
        .enumerate()
        .map(|(index, materialization)| {
            materialization
                .unwrap_or_else(|| panic!("missing {description} materialization at index {index}"))
        })
        .collect()
}
