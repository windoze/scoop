//! Initialization units are requested through their own typed source map.

use super::*;

mod generic_delegate;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct InitializationKey {
    pub(super) source: export::InitializationUnitId,
    pub(super) arguments: Vec<concrete::TypeId>,
}

pub(super) struct InitializationRequest {
    key: InitializationKey,
    failure_root: concrete::InitializationFailureRootId,
}

impl Concretizer<'_> {
    pub(super) fn request_initialization_unit(
        &mut self,
        source: export::InitializationUnitId,
    ) -> concrete::InitializationUnitId {
        self.request_initialization(InitializationKey {
            source,
            arguments: Vec::new(),
        })
    }

    fn request_initialization(&mut self, key: InitializationKey) -> concrete::InitializationUnitId {
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
        });
        self.pending_initializations.push_back(key);
        id
    }

    pub(super) fn require_initialization_dependencies(&mut self, key: InitializationKey) {
        let unit = &self.source.initialization_units[key.source];
        if let export::InitializationUnitKind::LazySingleton { value, .. } = unit.kind {
            self.lower_singleton_value(value);
        }
        self.request_function(unit.initializer, key.arguments.clone());
        self.request_function(unit.ensure, key.arguments);
        if let export::CoreProtocols::Defined(protocols) = self.core {
            self.request_function(
                protocols.exceptions.initialization_cycle_thrower,
                Vec::new(),
            );
        }
        for dependency in &unit.dependencies {
            self.request_initialization_unit(dependency.unit);
        }
    }

    pub(super) fn initialization_helper_is_required(&self, function: export::FunctionId) -> bool {
        self.initialization_function_units
            .get(&function)
            .is_none_or(|unit| {
                self.initialization_map.contains_key(&InitializationKey {
                    source: *unit,
                    arguments: Vec::new(),
                })
            })
    }

    pub(super) fn finish_initialization_units(
        &mut self,
        exact_types: &concrete::ExactTypeIdentities,
    ) {
        for request in &self.initialization_requests {
            let source = &self.source.initialization_units[request.key.source];
            let identity = self.initialization_identity(&request.key, exact_types);
            let kind = match source.kind {
                export::InitializationUnitKind::EagerTopLevel { storage, .. } => {
                    concrete::InitializationUnitKind::EagerTopLevel {
                        storage: self.global_map[&storage],
                    }
                }
                export::InitializationUnitKind::GenericDelegatedExtension { property, .. } => {
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
                    value: self.singleton_value_map[&value],
                    published_root: self.singleton_root_map[&published_root],
                },
            };
            let function =
                |source, arguments| self.function_by_key[&FunctionKey::Free { source, arguments }];
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
            let id = self
                .initialization_units
                .alloc(concrete::InitializationUnit {
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
                    dependencies: source
                        .dependencies
                        .iter()
                        .map(|dependency| concrete::InitializationDependency {
                            unit: self.initialization_map[&InitializationKey {
                                source: dependency.unit,
                                arguments: Vec::new(),
                            }],
                        })
                        .collect(),
                    cycle_thrower,
                });
            assert_eq!(id, self.initialization_map[&request.key]);
        }
    }

    fn initialization_identity(
        &self,
        key: &InitializationKey,
        exact_types: &concrete::ExactTypeIdentities,
    ) -> concrete::InitializationUnitIdentityRecord {
        if let export::InitializationUnitKind::GenericDelegatedExtension { property, .. } =
            self.source.initialization_units[key.source].kind
        {
            let property = self.source.property_identities[property]
                .extension_id()
                .expect("a generic delegate is an extension property");
            let receiver_arguments = scoop_identity::NonEmptyVec::new(
                key.arguments
                    .iter()
                    .map(|argument| exact_types[*argument].id())
                    .collect(),
            )
            .expect("a generic delegate retains nonempty receiver arguments");
            scoop_identity::CborIdentityRecord::from_key(
                scoop_identity::InitializationUnitKey::GenericDelegatedExtensionApplication {
                    property,
                    receiver_arguments,
                },
            )
            .expect("a delegate application has a canonical initialization identity")
        } else {
            assert!(key.arguments.is_empty());
            self.source.initialization_unit_identities[key.source].clone()
        }
    }
}
