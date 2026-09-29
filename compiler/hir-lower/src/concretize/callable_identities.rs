use std::collections::HashMap;

use scoop_identity::{
    CallableApplicationKey, CallableInstantiationOwner, CallableMaterialization,
    CallableMaterializationContext, CallableTemplateOwner, CallbackApplicationKey,
    CborIdentityRecord, GeneratedCallableKey, NonEmptyVec, PersistentCallableApplicationId,
    PersistentCallbackApplicationId, StructuralDefinitionPath,
};

use super::*;

mod constructors;
mod defaults;
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
    ) -> BuiltCallableIdentities {
        CallableIdentityBuilder::new(self, exact_types).build()
    }
}

pub(super) struct BuiltCallableIdentities {
    pub(super) default_local_values: Vec<concrete::DefaultLocalValueScope>,
    pub(super) callable_applications: concrete::CallableApplicationIdentities,
    pub(super) generated_callable_identities: Vec<concrete::GeneratedCallableRecord>,
    pub(super) callback_applications: concrete::CallbackApplicationIdentities,
    pub(super) function_materializations: Vec<CallableMaterialization>,
    pub(super) class_constructor_materializations: Vec<CallableMaterialization>,
    pub(super) struct_constructor_materializations: Vec<CallableMaterialization>,
    pub(super) callable_reference_identities: Vec<concrete::CallableReferenceIdentity>,
    pub(super) foreign_callback_applications: Vec<PersistentCallbackApplicationId>,
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
    callback_applications: Vec<concrete::CallbackApplicationRecord>,
    callback_application_by_key: HashMap<CallbackApplicationKey, PersistentCallbackApplicationId>,
    foreign_callback_applications: Vec<Option<PersistentCallbackApplicationId>>,
    generated_callable_identities: std::collections::BTreeMap<
        scoop_identity::PersistentGeneratedCallableId,
        concrete::GeneratedCallableRecord,
    >,
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
                    owner_type_parameter_count: declaration.owner_type_arguments.len(),
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
            callback_applications: Vec::new(),
            callback_application_by_key: HashMap::new(),
            foreign_callback_applications: vec![None; concretizer.foreign_callback_slots.len()],
            generated_callable_identities: std::collections::BTreeMap::new(),
        }
    }

    fn build(mut self) -> BuiltCallableIdentities {
        for index in 0..self.concretizer.function_keys.len() {
            self.resolve_function(index);
        }
        for index in 0..self.concretizer.class_constructor_keys.len() {
            self.resolve_class_constructor(index);
        }
        for index in 0..self.concretizer.struct_constructor_keys.len() {
            self.resolve_struct_constructor(index);
        }
        let callable_reference_identities = (0..self.concretizer.callable_reference_slots.len())
            .map(|index| self.resolve_callable_reference(index))
            .collect::<Vec<_>>();
        for index in 0..self.concretizer.foreign_callback_slots.len() {
            self.resolve_foreign_callback(index);
        }
        let default_local_values = self.materialize_default_local_values();
        let function_materializations = std::mem::take(&mut self.materializations)
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
        let generated_bodies = function_materializations
            .iter()
            .chain(&class_constructor_materializations)
            .chain(&struct_constructor_materializations)
            .chain(
                callable_reference_identities
                    .iter()
                    .map(concrete::CallableReferenceIdentity::materialization),
            )
            .filter_map(generated_body_member)
            .collect::<Vec<_>>();
        let callable_applications =
            concrete::CallableApplicationIdentities::checked_with_generated_bodies(
                self.applications,
                generated_bodies,
            )
            .expect(
                "validated concrete callable applications form one complete ODR identity graph",
            );
        let callback_applications =
            concrete::CallbackApplicationIdentities::checked(self.callback_applications)
                .expect("validated concrete callback applications form one canonical table");
        let foreign_callback_applications =
            complete_callback_applications(&mut self.foreign_callback_applications);
        BuiltCallableIdentities {
            default_local_values,
            callable_applications,
            generated_callable_identities: self
                .generated_callable_identities
                .into_values()
                .collect(),
            callback_applications,
            function_materializations,
            class_constructor_materializations,
            struct_constructor_materializations,
            callable_reference_identities,
            foreign_callback_applications,
        }
    }

    fn resolve_callable_reference(&mut self, index: usize) -> concrete::CallableReferenceIdentity {
        let pending = &self.concretizer.callable_reference_slots[index];
        let arguments = pending.owner_arguments.clone();
        let source = match &pending.source {
            CallableReferenceSource::Local(source) => *source,
            CallableReferenceSource::Imported { parent, definition } => {
                let parent = *parent;
                let definition = definition.clone();
                let enclosing = self.imported_parent_materialization(parent, &arguments);
                return concrete::CallableReferenceIdentity::from_record(
                    definition,
                    enclosing.context(),
                )
                .expect("an imported callable reference retains its provider invoke key");
            }
        };
        let source = &self.concretizer.source.callable_references[source];
        let root = source.definition_root;
        let path = source.definition_path.clone();
        let enclosing = self.enclosing_materialization(None, root, &path, &arguments);
        let parent = self.lexical_parent(enclosing);
        concrete::CallableReferenceIdentity::new(parent, path, enclosing.context())
            .expect("a validated callable-reference invoke key has a persistent identity")
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
        let source = match key.source() {
            FunctionSource::Local(source) => source,
            FunctionSource::Imported(source) => {
                let declaration = self.concretizer.source.imported_generic_templates[source]
                    .declaration
                    .clone();
                let arguments = self.concretizer.function_key_arguments(&key);
                let owner = match key {
                    FunctionKey::ImportedMethod { owner, .. } => Some(owner),
                    _ => None,
                };
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
                let exact_owner = self.exact_method_owner(match key {
                    FunctionKey::Method { owner, .. } => owner,
                    FunctionKey::Free { .. }
                    | FunctionKey::Imported { .. }
                    | FunctionKey::ImportedMethod { .. } => {
                        panic!("derived equality is always an exact-owner method")
                    }
                });
                let record = CborIdentityRecord::from_key(GeneratedCallableKey::DerivedEquality {
                    exact_owner,
                })
                .expect("a derived equality has its complete exact owner");
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

    fn resolve_foreign_callback(&mut self, index: usize) -> PersistentCallbackApplicationId {
        if let Some(application) = self.foreign_callback_applications[index] {
            return application;
        }
        let pending = &self.concretizer.foreign_callback_slots[index];
        let registration =
            &self.concretizer.source.callback_registration_identities[pending.source];
        let context = self.callback_context(pending.source, &pending.arguments);
        let key = CallbackApplicationKey::new(registration.key(), context)
            .expect("a concrete callback site has a complete materialization context");
        let application = self.record_callback_application(key);
        self.foreign_callback_applications[index] = Some(application);
        application
    }

    fn declaration_materialization(
        &mut self,
        key: &FunctionKey,
        template: SourceTemplate,
    ) -> CallableMaterialization {
        match key {
            FunctionKey::ImportedMethod { .. } => {
                unreachable!("imported methods use their provider identity")
            }
            FunctionKey::Free { arguments, .. } | FunctionKey::Imported { arguments, .. } => self
                .source_materialization(template, CallableInstantiationOwner::NoOwner, arguments),
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

    fn record_callback_application(
        &mut self,
        key: CallbackApplicationKey,
    ) -> PersistentCallbackApplicationId {
        if let Some(application) = self.callback_application_by_key.get(&key) {
            return *application;
        }
        let record = CborIdentityRecord::from_key(key)
            .expect("validated callback application keys are hashable");
        let application = record.id();
        self.callback_application_by_key
            .insert(record.key().to_owned(), application);
        self.callback_applications.push(record);
        application
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

fn complete_callback_applications(
    applications: &mut Vec<Option<PersistentCallbackApplicationId>>,
) -> Vec<PersistentCallbackApplicationId> {
    std::mem::take(applications)
        .into_iter()
        .enumerate()
        .map(|(index, application)| {
            application
                .unwrap_or_else(|| panic!("missing foreign callback application at index {index}"))
        })
        .collect()
}

fn generated_body_member(
    materialization: &CallableMaterialization,
) -> Option<(
    PersistentCallableApplicationId,
    scoop_identity::PersistentGeneratedCallableId,
)> {
    let CallableTemplateOwner::Generated(generated) = materialization.template() else {
        return None;
    };
    let CallableMaterializationContext::Application(application) = materialization.context() else {
        return None;
    };
    Some((application, generated))
}
