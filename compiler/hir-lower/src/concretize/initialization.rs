//! Initialization units are requested through their own typed source map.

use super::*;

pub(super) struct InitializationRequest {
    source: export::InitializationUnitId,
    failure_root: concrete::InitializationFailureRootId,
}

impl Concretizer<'_> {
    pub(super) fn request_initialization_unit(
        &mut self,
        source: export::InitializationUnitId,
    ) -> concrete::InitializationUnitId {
        if let Some(id) = self.initialization_map.get(&source) {
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
        self.initialization_map.insert(source, id);
        self.initialization_requests.push(InitializationRequest {
            source,
            failure_root,
        });
        self.pending_initializations.push_back(source);
        id
    }

    pub(super) fn require_initialization_dependencies(
        &mut self,
        source: export::InitializationUnitId,
    ) {
        let unit = &self.source.initialization_units[source];
        if let export::InitializationUnitKind::LazySingleton { value, .. } = unit.kind {
            self.lower_singleton_value(value);
        }
        self.request_function(unit.initializer, Vec::new());
        self.request_function(unit.ensure, Vec::new());
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
            .is_none_or(|unit| self.initialization_map.contains_key(unit))
    }

    pub(super) fn finish_initialization_units(&mut self) {
        for request in &self.initialization_requests {
            let source = &self.source.initialization_units[request.source];
            let kind = match source.kind {
                export::InitializationUnitKind::EagerTopLevel { storage, .. } => {
                    concrete::InitializationUnitKind::EagerTopLevel {
                        storage: self.global_map[&storage],
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
            let function = |source| {
                self.function_by_key[&FunctionKey::Free {
                    source,
                    arguments: Vec::new(),
                }]
            };
            let cycle_thrower = match self.core {
                export::CoreProtocols::Defined(protocols) => {
                    concrete::InitializationCycleThrower::Local(function(
                        protocols.exceptions.initialization_cycle_thrower,
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
                    identity: self.source.initialization_unit_identities[request.source].clone(),
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
                    initializer: function(source.initializer),
                    ensure: function(source.ensure),
                    failure_root: request.failure_root,
                    dependencies: source
                        .dependencies
                        .iter()
                        .map(|dependency| concrete::InitializationDependency {
                            unit: self.initialization_map[&dependency.unit],
                        })
                        .collect(),
                    cycle_thrower,
                });
            assert_eq!(id, self.initialization_map[&request.source]);
        }
    }
}
