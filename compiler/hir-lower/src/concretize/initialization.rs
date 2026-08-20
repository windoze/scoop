//! Initialization units are requested through their own typed source map.

use super::*;

mod companions;
mod generic_delegate;
mod identity;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct InitializationKey {
    pub(super) source: InitializationSource,
    pub(super) arguments: Vec<concrete::TypeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum InitializationSource {
    Defined(export::InitializationUnitId),
    ImportedDelegate(export::ImportedGenericDelegateTemplateId),
    ImportedCompanion(export::ImportedCompanionTemplateId),
}

pub(super) struct InitializationRequest {
    key: InitializationKey,
    failure_root: concrete::InitializationFailureRootId,
    dependencies: Vec<concrete::InitializationUnitId>,
}

impl Concretizer<'_> {
    pub(super) fn request_initialization_unit(
        &mut self,
        source: export::InitializationUnitId,
    ) -> concrete::InitializationUnitId {
        self.request_initialization(InitializationKey {
            source: InitializationSource::Defined(source),
            arguments: Vec::new(),
        })
    }

    pub(super) fn request_initialization(
        &mut self,
        key: InitializationKey,
    ) -> concrete::InitializationUnitId {
        if let Some(id) = self.initialization_map.get(&key) {
            return *id;
        }
        let id = concrete::InitializationUnitId::from_raw(
            u32::try_from(self.initialization_requests.len())
                .expect("initialization units fit their typed id domain")
                .into(),
        );
        let failure_root = self
            .initialization_failure_roots
            .alloc(concrete::InitializationFailureRoot { unit: id });
        self.initialization_map.insert(key.clone(), id);
        self.initialization_requests.push(InitializationRequest {
            key: key.clone(),
            failure_root,
            dependencies: Vec::new(),
        });
        self.pending_initializations.push_back(key);
        id
    }

    pub(super) fn require_initialization_dependencies(&mut self, key: InitializationKey) {
        let mut dependencies = Vec::new();
        match key.source {
            InitializationSource::ImportedCompanion(template) => {
                for ty in &self.source.imported_companion_templates[template].dependencies {
                    let ty = self.lower_type(*ty, &key.arguments);
                    let value = self.singleton_value_for_type(ty);
                    dependencies.push(self.singleton_values[value].initialization);
                }
                for role in [
                    scoop_identity::InitializationCallableRole::Initializer,
                    scoop_identity::InitializationCallableRole::Ensure,
                ] {
                    let source = FunctionSource::Companion(template, role);
                    let function = self.function_key(source, None, key.arguments.clone());
                    self.request_function_key(function, source);
                }
            }
            InitializationSource::Defined(source) => {
                let unit = &self.source.initialization_units[source];
                if let export::InitializationUnitKind::LazySingleton { value, .. } = unit.kind {
                    self.lower_singleton_value(value, &key.arguments);
                }
                self.request_function(unit.initializer, key.arguments.clone());
                self.request_function(unit.ensure, key.arguments.clone());
                for dependency in &unit.dependencies {
                    let arguments = dependency
                        .type_arguments
                        .iter()
                        .map(|ty| self.lower_type(*ty, &key.arguments))
                        .collect();
                    dependencies.push(self.request_initialization(InitializationKey {
                        source: InitializationSource::Defined(dependency.unit),
                        arguments,
                    }));
                }
            }
            InitializationSource::ImportedDelegate(template) => {
                let template = &self.source.imported_generic_delegate_templates[template];
                for source in [template.initializer, template.ensure] {
                    let source = FunctionSource::Imported(source);
                    let function = self.function_key(source, None, key.arguments.clone());
                    self.request_function_key(function, source);
                }
            }
        }
        let request = self.initialization_map[&key].into_raw().into_u32() as usize;
        self.initialization_requests[request].dependencies = dependencies;
        if let export::CoreProtocols::Defined(protocols) = self.core {
            self.request_function(
                protocols.exceptions.initialization_cycle_thrower,
                Vec::new(),
            );
        }
    }

    pub(super) fn initialization_helper_is_required(&self, function: export::FunctionId) -> bool {
        self.initialization_function_units
            .get(&function)
            .is_none_or(|unit| {
                self.initialization_map
                    .keys()
                    .any(|key| key.source == InitializationSource::Defined(*unit))
            })
    }

    pub(super) fn finish_initialization_units(
        &mut self,
        exact_types: &concrete::ExactTypeIdentities,
    ) {
        for request in &self.initialization_requests {
            let identity = self.initialization_identity(&request.key, exact_types);
            let function = |source, arguments| {
                self.function_by_key
                    [&self.function_key(FunctionSource::Local(source), None, arguments)]
            };
            let cycle_thrower = match self.core {
                export::CoreProtocols::Defined(protocols) => {
                    concrete::InitializationCycleThrower::Local(function(
                        protocols.exceptions.initialization_cycle_thrower,
                        Vec::new(),
                    ))
                }
                export::CoreProtocols::Imported(protocols) => {
                    concrete::InitializationCycleThrower::Imported(
                        protocols
                            .exceptions()
                            .initialization_cycle_thrower()
                            .clone(),
                    )
                }
            };
            let unit = match request.key.source {
                InitializationSource::ImportedCompanion(template) => {
                    self.finish_companion_initialization(request, template, identity, cycle_thrower)
                }
                InitializationSource::Defined(source) => {
                    let source = &self.source.initialization_units[source];
                    let kind = match source.kind {
                        export::InitializationUnitKind::EagerTopLevel { storage, .. } => {
                            concrete::InitializationUnitKind::EagerTopLevel {
                                storage: self.global_map[&storage],
                            }
                        }
                        export::InitializationUnitKind::GenericDelegatedExtension {
                            property,
                            ..
                        } => {
                            let property = self.source.property_identities[property]
                                .extension_id()
                                .expect("a generic delegate is an extension property");
                            concrete::InitializationUnitKind::GenericDelegatedExtension {
                                specialization: self.generic_delegate_by_key
                                    [&(property, request.key.arguments.clone())],
                            }
                        }
                        export::InitializationUnitKind::LazySingleton {
                            value,
                            published_root,
                        } => concrete::InitializationUnitKind::LazySingleton {
                            value: self
                                .singleton_value_for_application(value, &request.key.arguments),
                            published_root: self.singleton_root_map[&self
                                .singleton_value_for_application(
                                    self.source.singleton_published_roots[published_root].value,
                                    &request.key.arguments,
                                )],
                        },
                    };
                    concrete::InitializationUnit {
                        identity,
                        display_name: source.display_name.clone(),
                        schedule: match source.schedule {
                            export::InitializationSchedule::EagerStartup => {
                                concrete::InitializationSchedule::EagerStartup
                            }
                            export::InitializationSchedule::LazyAccess => {
                                concrete::InitializationSchedule::LazyAccess
                            }
                        },
                        kind,
                        initializer: function(source.initializer, request.key.arguments.clone()),
                        ensure: function(source.ensure, request.key.arguments.clone()),
                        failure_root: request.failure_root,
                        dependencies: request
                            .dependencies
                            .iter()
                            .map(|unit| concrete::InitializationDependency { unit: *unit })
                            .collect(),
                        cycle_thrower,
                    }
                }
                InitializationSource::ImportedDelegate(template) => {
                    let template = &self.source.imported_generic_delegate_templates[template];
                    let function = |source| {
                        self.function_by_key[&self.function_key(
                            FunctionSource::Imported(source),
                            None,
                            request.key.arguments.clone(),
                        )]
                    };
                    concrete::InitializationUnit {
                        identity,
                        display_name: template.diagnostic_path.clone(),
                        schedule: concrete::InitializationSchedule::LazyAccess,
                        kind: concrete::InitializationUnitKind::GenericDelegatedExtension {
                            specialization: self.generic_delegate_by_key
                                [&(template.property, request.key.arguments.clone())],
                        },
                        initializer: function(template.initializer),
                        ensure: function(template.ensure),
                        failure_root: request.failure_root,
                        dependencies: Vec::new(),
                        cycle_thrower,
                    }
                }
            };
            let id = self.initialization_units.alloc(unit);
            assert_eq!(id, self.initialization_map[&request.key]);
        }
    }
}
