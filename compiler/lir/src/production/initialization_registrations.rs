//! Complete semantic plans for strong initialization-unit registrations.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub use scoop_identity::PersistentInitializationUnitId;
use scoop_identity::{
    CallableBodyKey, DecodedCallableBodyKey, DecodedCallableBodyKeyKind, DefinitionOwner,
    GeneratedCallableIdentityError, GeneratedCallableKey, InitializationCallableRole,
    InitializationUnitKey, NominalDeclarationOwner, NominalOwner, PersistentCallableBodyId,
    PersistentGeneratedCallableId, PersistentStaticStorageId, PropertyOwner, StaticStorageKey,
    StorageRole, StrongCallableDefinitionOwner,
};
use scoop_wire::{HashError, RuntimeDecodeError, decode_runtime};

use crate::{
    GcEffect, GlobalInit, InitializationSchedule, InitializationUnit, InitializationUnitId,
    InitializationUnitKind, LirTargetProfile, ManagedLocalFunctionRef, Module, RefScan,
    StrongStaticStorageInitialStatePlanV1, StrongStaticStorageSemanticPlanSetV1,
    StrongStaticStorageSemanticPlanV1,
};

mod model;
mod producer_v2;
mod unit_validation;
pub use model::*;
pub use producer_v2::*;
use unit_validation::*;

impl StrongInitializationUnitSemanticPlanSetV1 {
    pub fn from_module(
        module: &Module,
    ) -> Result<Self, StrongInitializationUnitSemanticPlanBuildError> {
        let static_storages = StrongStaticStorageSemanticPlanSetV1::from_module(module)
            .map_err(StrongInitializationUnitSemanticPlanBuildError::StaticStorage)?;
        Self::from_parts(
            module.cone,
            module.meta.target_profile,
            &module.globals,
            &module.initialization_units,
            &module.functions,
            static_storages,
        )
    }

    fn from_parts(
        producer: scoop_identity::ConeIdentity,
        target: LirTargetProfile,
        globals: &la_arena::Arena<crate::Global>,
        units: &la_arena::Arena<InitializationUnit>,
        functions: &[crate::Function],
        static_storages: StrongStaticStorageSemanticPlanSetV1,
    ) -> Result<Self, StrongInitializationUnitSemanticPlanBuildError> {
        if static_storages.producer() != producer {
            return Err(
                StrongInitializationUnitSemanticPlanBuildError::StaticStorageProducerMismatch {
                    expected: producer,
                    actual: static_storages.producer(),
                },
            );
        }

        let mut arena_units = Vec::with_capacity(units.len());
        let mut unit_ids = BTreeSet::new();
        for (arena_id, unit) in units.iter() {
            let id = unit.identity.id();
            if !unit_ids.insert(id) {
                return Err(StrongInitializationUnitSemanticPlanBuildError::DuplicateUnit(id));
            }
            arena_units.push((arena_id, id));
        }

        let expected_gateways = units
            .iter()
            .filter(|(_, unit)| matches!(unit.schedule, InitializationSchedule::EagerStartup))
            .map(|(_, unit)| {
                startup_gateway_body(unit.identity.id())
                    .map(|gateway| (gateway, unit.identity.id()))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let mut actual_gateways = BTreeMap::new();
        for function in functions {
            let body = function.callable_body.id();
            let key = decode_runtime::<DecodedCallableBodyKey>(
                function.callable_body.identity_record().key_bytes(),
            )
            .map_err(|error| {
                StrongInitializationUnitSemanticPlanBuildError::InvalidCallableBodyKey {
                    body,
                    error,
                }
            })?;
            let DecodedCallableBodyKeyKind::InitializationStartupGateway(unit) = key.kind() else {
                continue;
            };
            if !matches!(
                expected_gateways.get(&body),
                Some(expected) if expected.as_array() == unit.as_array()
            ) {
                return Err(
                    StrongInitializationUnitSemanticPlanBuildError::UnexpectedStartupGateway {
                        gateway: body,
                    },
                );
            }
            actual_gateways
                .entry(body)
                .or_insert_with(Vec::new)
                .push(function);
        }
        for gateway in expected_gateways.keys() {
            let actual = actual_gateways
                .get(gateway)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let [function] = actual else {
                return Err(
                    StrongInitializationUnitSemanticPlanBuildError::StartupGatewaySet {
                        gateway: *gateway,
                        actual: actual.len(),
                    },
                );
            };
            if function.gc_effect != GcEffect::Managed {
                return Err(
                    StrongInitializationUnitSemanticPlanBuildError::StartupGatewayEffect {
                        gateway: *gateway,
                        actual: function.gc_effect,
                    },
                );
            }
        }

        let mut plans = BTreeMap::new();
        for (arena_id, unit) in units.iter() {
            let plan = build_unit(
                target,
                globals,
                functions,
                &static_storages,
                &arena_units,
                arena_id,
                unit,
            )?;
            let id = plan.unit();
            if plans.insert(id, plan).is_some() {
                return Err(StrongInitializationUnitSemanticPlanBuildError::DuplicateUnit(id));
            }
        }
        Ok(Self {
            static_storages,
            units: plans.into_values().collect(),
        })
    }
}

fn build_unit(
    target: LirTargetProfile,
    globals: &la_arena::Arena<crate::Global>,
    functions: &[crate::Function],
    static_storages: &StrongStaticStorageSemanticPlanSetV1,
    arena_units: &[(InitializationUnitId, PersistentInitializationUnitId)],
    arena_id: InitializationUnitId,
    unit: &InitializationUnit,
) -> Result<StrongInitializationUnitSemanticPlanV1, StrongInitializationUnitSemanticPlanBuildError>
{
    let id = unit.identity.id();
    if unit.display_name.is_empty() {
        return Err(StrongInitializationUnitSemanticPlanBuildError::EmptyDiagnosticPath(id));
    }
    validate_unit_kind_and_schedule(unit)?;

    let storage_global = unit.kind.storage();
    if storage_global.into_raw().into_u32() as usize >= globals.len() {
        return Err(
            StrongInitializationUnitSemanticPlanBuildError::MissingGlobal {
                unit: id,
                global: storage_global,
            },
        );
    }
    let failure_global = unit.failure_root;
    if failure_global.into_raw().into_u32() as usize >= globals.len() {
        return Err(
            StrongInitializationUnitSemanticPlanBuildError::MissingGlobal {
                unit: id,
                global: failure_global,
            },
        );
    }
    if storage_global == failure_global {
        return Err(StrongInitializationUnitSemanticPlanBuildError::AliasedStorage(id));
    }
    let storage = require_storage(globals, static_storages, id, storage_global)?;
    let failure = require_storage(globals, static_storages, id, failure_global)?;
    require_zeroed(id, storage, InitializationStorageRoleV1::Value)?;
    require_zeroed(id, failure, InitializationStorageRoleV1::FailureRoot)?;
    validate_value_storage_key(id, unit, &globals[storage_global])?;
    validate_failure_root_key(id, &globals[failure_global])?;
    validate_failure_shape(target, id, failure)?;

    let (declaration_unit, group) = match unit.identity.key() {
        InitializationUnitKey::GenericDelegatedExtensionApplication { property, .. } => (
            PersistentInitializationUnitId::from_key(&InitializationUnitKey::ExtensionProperty(
                *property,
            ))
            .map_err(StrongInitializationUnitSemanticPlanBuildError::Hash)?,
            Some(
                scoop_identity::OdrGroupId::from_key(
                    &unit
                        .identity
                        .key()
                        .specialization_key()
                        .expect("a delegated unit has a specialization key"),
                )
                .map_err(StrongInitializationUnitSemanticPlanBuildError::Hash)?,
            ),
        ),
        _ => (id, None),
    };
    let initializer = generated_unit_body(
        declaration_unit,
        InitializationCallableRole::Initializer,
        group,
    )?;
    let ensure = generated_unit_body(declaration_unit, InitializationCallableRole::Ensure, group)?;
    if unit.initializer == unit.ensure || initializer == ensure {
        return Err(StrongInitializationUnitSemanticPlanBuildError::AliasedCallable(id));
    }
    validate_function_reference(id, functions, unit.initializer, initializer)?;
    validate_function_reference(id, functions, unit.ensure, ensure)?;

    let schedule = match unit.schedule {
        InitializationSchedule::EagerStartup => StrongInitializationSchedulePlanV1::EagerStartup {
            gateway: startup_gateway_body(id)?,
        },
        InitializationSchedule::LazyAccess => StrongInitializationSchedulePlanV1::LazyAccess,
    };
    let mut dependencies = Vec::with_capacity(unit.dependencies.len());
    for dependency in &unit.dependencies {
        let Some((_, persistent)) = arena_units
            .iter()
            .find(|(candidate, _)| candidate == dependency)
        else {
            return Err(
                StrongInitializationUnitSemanticPlanBuildError::MissingDependency {
                    unit: id,
                    dependency: *dependency,
                },
            );
        };
        if *dependency == arena_id {
            return Err(StrongInitializationUnitSemanticPlanBuildError::SelfDependency(id));
        }
        dependencies.push(*persistent);
    }
    dependencies.sort_unstable();
    if dependencies.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(StrongInitializationUnitSemanticPlanBuildError::DuplicateDependency(id));
    }

    Ok(StrongInitializationUnitSemanticPlanV1 {
        unit: id,
        diagnostic_path: unit.display_name.clone(),
        schedule,
        storage: storage.storage(),
        failure_root: failure.storage(),
        initializer,
        ensure,
        dependencies,
    })
}

pub(crate) fn generated_unit_body(
    unit: PersistentInitializationUnitId,
    role: InitializationCallableRole,
    group: Option<scoop_identity::OdrGroupId>,
) -> Result<PersistentCallableBodyId, StrongInitializationUnitSemanticPlanBuildError> {
    let generated =
        PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
            unit,
            role,
        })
        .map_err(StrongInitializationUnitSemanticPlanBuildError::GeneratedCallableIdentity)?;
    let key = if let Some(group) = group {
        let member = scoop_identity::OdrMemberKey::new(
            group,
            scoop_identity::OdrMemberRole::CallableBody,
            scoop_identity::OdrMemberDiscriminator::GeneratedCallable(generated),
        )
        .and_then(|key| scoop_identity::CallableOdrMemberId::from_key(&key))
        .map_err(StrongInitializationUnitSemanticPlanBuildError::OdrMember)?;
        CallableBodyKey::odr(member)
    } else {
        CallableBodyKey::strong(StrongCallableDefinitionOwner::GeneratedCallable(generated))
    };
    PersistentCallableBodyId::from_key(&key)
        .map_err(StrongInitializationUnitSemanticPlanBuildError::Hash)
}

pub(crate) fn startup_gateway_body(
    unit: PersistentInitializationUnitId,
) -> Result<PersistentCallableBodyId, StrongInitializationUnitSemanticPlanBuildError> {
    PersistentCallableBodyId::from_key(&CallableBodyKey::initialization_startup_gateway(unit))
        .map_err(StrongInitializationUnitSemanticPlanBuildError::Hash)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationStorageRoleV1 {
    Value,
    FailureRoot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongInitializationUnitSemanticPlanBuildError {
    StaticStorage(crate::StrongStaticStorageSemanticPlanBuildError),
    StaticStorageProducerMismatch {
        expected: scoop_identity::ConeIdentity,
        actual: scoop_identity::ConeIdentity,
    },
    DuplicateUnit(PersistentInitializationUnitId),
    OdrMember(scoop_identity::OdrMemberIdentityError),
    EmptyDiagnosticPath(PersistentInitializationUnitId),
    KindSchedule {
        unit: PersistentInitializationUnitId,
    },
    MissingGlobal {
        unit: PersistentInitializationUnitId,
        global: crate::GlobalId,
    },
    AliasedStorage(PersistentInitializationUnitId),
    MissingStaticStorage {
        unit: PersistentInitializationUnitId,
        global: crate::GlobalId,
    },
    NonZeroedStorage {
        unit: PersistentInitializationUnitId,
        storage: PersistentStaticStorageId,
        role: InitializationStorageRoleV1,
    },
    ValueStorageKey(PersistentInitializationUnitId),
    FailureRootKey(PersistentInitializationUnitId),
    FailureRootShape {
        unit: PersistentInitializationUnitId,
        storage: PersistentStaticStorageId,
    },
    MissingFunctionReference {
        unit: PersistentInitializationUnitId,
        index: usize,
    },
    FunctionReferenceEffect {
        unit: PersistentInitializationUnitId,
        body: PersistentCallableBodyId,
        actual: GcEffect,
    },
    FunctionReferenceIdentity {
        unit: PersistentInitializationUnitId,
        expected: PersistentCallableBodyId,
        actual: PersistentCallableBodyId,
    },
    FunctionBodySet {
        unit: PersistentInitializationUnitId,
        body: PersistentCallableBodyId,
        actual: usize,
    },
    AliasedCallable(PersistentInitializationUnitId),
    StartupGatewaySet {
        gateway: PersistentCallableBodyId,
        actual: usize,
    },
    UnexpectedStartupGateway {
        gateway: PersistentCallableBodyId,
    },
    InvalidCallableBodyKey {
        body: PersistentCallableBodyId,
        error: RuntimeDecodeError,
    },
    StartupGatewayEffect {
        gateway: PersistentCallableBodyId,
        actual: GcEffect,
    },
    MissingDependency {
        unit: PersistentInitializationUnitId,
        dependency: InitializationUnitId,
    },
    SelfDependency(PersistentInitializationUnitId),
    DuplicateDependency(PersistentInitializationUnitId),
    GeneratedCallableIdentity(GeneratedCallableIdentityError),
    Hash(HashError),
}

impl fmt::Display for StrongInitializationUnitSemanticPlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong initialization-unit semantics: {self:?}"
        )
    }
}

impl std::error::Error for StrongInitializationUnitSemanticPlanBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::StaticStorage(source) => Some(source),
            Self::InvalidCallableBodyKey { error, .. } => Some(error),
            Self::GeneratedCallableIdentity(source) => Some(source),
            Self::Hash(source) => Some(source),
            _ => None,
        }
    }
}

mod registrations;
#[cfg(test)]
pub(in crate::production) use registrations::tests::external_dependency_for_layout_join;
pub use registrations::*;

#[cfg(test)]
mod tests;
