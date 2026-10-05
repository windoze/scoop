use std::collections::HashMap;

use scoop_identity::{
    CallableApplicationKey, CallableInstantiationOwner, CallableMaterialization,
    CallableMaterializationContext, CallableTemplateOwner, CallbackApplicationKey,
    CborIdentityRecord, GeneratedCallableKey, NonEmptyVec, PersistentCallableApplicationId,
    PersistentCallbackApplicationId, StructuralDefinitionPath,
};

use super::*;

mod common_values;
mod constructors;
mod defaults;
mod functions;
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
    pub(super) lexical_local_values: Vec<concrete::LexicalLocalValueScope>,
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
            let Some((source_function, source_root)) = declaration.source() else {
                continue;
            };
            insert_lexical_site(
                &mut lexical_sites,
                source_function,
                LexicalSite {
                    root: source_root,
                    path: declaration.definition_path.clone(),
                    owner_type_parameter_count: declaration.owner_type_param_count,
                },
            );
        }
        for (_, declaration) in concretizer.source.lambdas.iter() {
            let Some((source_function, source_root)) = declaration.definition.source() else {
                continue;
            };
            insert_lexical_site(
                &mut lexical_sites,
                source_function,
                LexicalSite {
                    root: source_root,
                    path: declaration.definition_path.clone(),
                    owner_type_parameter_count: declaration.owner_type_param_count,
                },
            );
        }
        for (_, declaration) in concretizer.source.anonymous_functions.iter() {
            let Some((source_function, source_root)) = declaration.definition.source() else {
                continue;
            };
            insert_lexical_site(
                &mut lexical_sites,
                source_function,
                LexicalSite {
                    root: source_root,
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
                concretizer.class_constructor_definitions.len()
            ],
            struct_constructor_materializations: vec![
                None;
                concretizer
                    .struct_constructor_definitions
                    .len()
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
        for index in 0..self.concretizer.class_constructor_definitions.len() {
            self.resolve_class_constructor(index);
        }
        for index in 0..self.concretizer.struct_constructor_definitions.len() {
            self.resolve_struct_constructor(index);
        }
        let callable_reference_identities = (0..self.concretizer.callable_reference_slots.len())
            .map(|index| self.resolve_callable_reference(index))
            .collect::<Vec<_>>();
        for index in 0..self.concretizer.foreign_callback_slots.len() {
            self.resolve_foreign_callback(index);
        }
        let mut lexical_local_values = self.materialize_default_local_values();
        lexical_local_values.extend(self.materialize_common_initialization_values());
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
            lexical_local_values,
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
        let source = &self.concretizer.source.callable_references[pending.source];
        let path = source.definition_path.clone();
        let (parent, enclosing) = match source.definition_root {
            export::CallableReferenceRoot::Source(root) => {
                let enclosing = self.enclosing_materialization(None, root, &path, &arguments);
                (self.lexical_parent(enclosing), enclosing)
            }
            export::CallableReferenceRoot::Persistent(parent) => (
                parent,
                self.imported_parent_materialization(parent.template(), &arguments),
            ),
        };
        concrete::CallableReferenceIdentity::new(parent, path, enclosing.context())
            .expect("a validated callable-reference invoke key has a persistent identity")
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
        let Some(owner) = key.owner else {
            return self.source_materialization(
                template,
                CallableInstantiationOwner::NoOwner,
                &key.arguments,
            );
        };
        let owner_arguments = self.concretizer.concrete_method_owner_arguments(owner);
        let instantiation_owner = if owner_arguments.is_empty() {
            CallableInstantiationOwner::NoOwner
        } else {
            CallableInstantiationOwner::ExactNominalOwner(self.exact_method_owner(owner))
        };
        self.source_materialization(
            template,
            instantiation_owner,
            &key.arguments[owner_arguments.len()..],
        )
    }

    fn local_source_materialization(
        &mut self,
        key: &FunctionKey,
        template: SourceTemplate,
        site: &LexicalSite,
    ) -> CallableMaterialization {
        assert!(
            key.owner.is_none(),
            "a block-local callable is not a nominal method"
        );
        let FunctionSource::Local(source) = self.concretizer.function_source(key) else {
            unreachable!("a current lexical site retains its source record")
        };
        let arguments = &key.arguments;
        assert!(site.owner_type_parameter_count <= arguments.len());
        let inherited = &arguments[..site.owner_type_parameter_count];
        let own = &arguments[site.owner_type_parameter_count..];
        let owner = match self.lexical_context(source, site, inherited) {
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
