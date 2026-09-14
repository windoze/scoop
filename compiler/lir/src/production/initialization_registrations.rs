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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongInitializationSchedulePlanV1 {
    EagerStartup { gateway: PersistentCallableBodyId },
    LazyAccess,
}

impl StrongInitializationSchedulePlanV1 {
    pub const fn tag(self) -> u32 {
        match self {
            Self::EagerStartup { .. } => 1,
            Self::LazyAccess => 2,
        }
    }

    pub const fn gateway(self) -> Option<PersistentCallableBodyId> {
        match self {
            Self::EagerStartup { gateway } => Some(gateway),
            Self::LazyAccess => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitSemanticPlanV1 {
    unit: PersistentInitializationUnitId,
    diagnostic_path: String,
    schedule: StrongInitializationSchedulePlanV1,
    storage: PersistentStaticStorageId,
    failure_root: PersistentStaticStorageId,
    initializer: PersistentCallableBodyId,
    ensure: PersistentCallableBodyId,
    dependencies: Vec<PersistentInitializationUnitId>,
}

impl StrongInitializationUnitSemanticPlanV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_artifact(
        unit: PersistentInitializationUnitId,
        diagnostic_path: String,
        schedule: StrongInitializationSchedulePlanV1,
        storage: PersistentStaticStorageId,
        failure_root: PersistentStaticStorageId,
        initializer: PersistentCallableBodyId,
        ensure: PersistentCallableBodyId,
        dependencies: Vec<PersistentInitializationUnitId>,
    ) -> Self {
        Self {
            unit,
            diagnostic_path,
            schedule,
            storage,
            failure_root,
            initializer,
            ensure,
            dependencies,
        }
    }

    pub const fn unit(&self) -> PersistentInitializationUnitId {
        self.unit
    }

    pub fn diagnostic_path(&self) -> &str {
        &self.diagnostic_path
    }

    pub const fn schedule(&self) -> StrongInitializationSchedulePlanV1 {
        self.schedule
    }

    pub const fn storage(&self) -> PersistentStaticStorageId {
        self.storage
    }

    pub const fn failure_root(&self) -> PersistentStaticStorageId {
        self.failure_root
    }

    pub const fn initializer(&self) -> PersistentCallableBodyId {
        self.initializer
    }

    pub const fn ensure(&self) -> PersistentCallableBodyId {
        self.ensure
    }

    pub fn dependencies(&self) -> &[PersistentInitializationUnitId] {
        &self.dependencies
    }
}

/// Proof that every final-LIR initialization unit has one complete semantic
/// plan and that no startup gateway exists outside the eager-unit set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongInitializationUnitSemanticPlanSetV1 {
    static_storages: StrongStaticStorageSemanticPlanSetV1,
    units: Vec<StrongInitializationUnitSemanticPlanV1>,
}

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

    pub(crate) const fn from_artifact(
        static_storages: StrongStaticStorageSemanticPlanSetV1,
        units: Vec<StrongInitializationUnitSemanticPlanV1>,
    ) -> Self {
        Self {
            static_storages,
            units,
        }
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
            if unit.identity.key().specialization_key().is_some() {
                return Err(StrongInitializationUnitSemanticPlanBuildError::OdrUnit(id));
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
            *actual_gateways.entry(body).or_insert(0usize) += 1;
        }
        for gateway in expected_gateways.keys() {
            let actual = actual_gateways.get(gateway).copied().unwrap_or(0);
            if actual != 1 {
                return Err(
                    StrongInitializationUnitSemanticPlanBuildError::StartupGatewaySet {
                        gateway: *gateway,
                        actual,
                    },
                );
            }
            let function = functions
                .iter()
                .find(|function| function.callable_body.id() == *gateway)
                .expect("the gateway count was verified as one");
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

    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.static_storages.producer()
    }

    pub const fn static_storages(&self) -> &StrongStaticStorageSemanticPlanSetV1 {
        &self.static_storages
    }

    pub fn units(&self) -> &[StrongInitializationUnitSemanticPlanV1] {
        &self.units
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

    let initializer = generated_unit_body(id, InitializationCallableRole::Initializer)?;
    let ensure = generated_unit_body(id, InitializationCallableRole::Ensure)?;
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

fn require_storage<'storage>(
    globals: &la_arena::Arena<crate::Global>,
    storages: &'storage StrongStaticStorageSemanticPlanSetV1,
    unit: PersistentInitializationUnitId,
    global: crate::GlobalId,
) -> Result<
    &'storage StrongStaticStorageSemanticPlanV1,
    StrongInitializationUnitSemanticPlanBuildError,
> {
    let GlobalInit::Storage { identity, .. } = &globals[global].init else {
        return Err(
            StrongInitializationUnitSemanticPlanBuildError::MissingStaticStorage { unit, global },
        );
    };
    let storage = identity.identity_record().id();
    storages
        .storages()
        .iter()
        .find(|candidate| candidate.storage() == storage)
        .ok_or(
            StrongInitializationUnitSemanticPlanBuildError::MissingStaticStorage { unit, global },
        )
}

fn require_zeroed(
    unit: PersistentInitializationUnitId,
    storage: &StrongStaticStorageSemanticPlanV1,
    role: InitializationStorageRoleV1,
) -> Result<(), StrongInitializationUnitSemanticPlanBuildError> {
    matches!(
        storage.initial_state(),
        StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit
    )
    .then_some(())
    .ok_or(
        StrongInitializationUnitSemanticPlanBuildError::NonZeroedStorage {
            unit,
            storage: storage.storage(),
            role,
        },
    )
}

fn validate_unit_kind_and_schedule(
    unit: &InitializationUnit,
) -> Result<(), StrongInitializationUnitSemanticPlanBuildError> {
    let id = unit.identity.id();
    let valid = matches!(
        (unit.identity.key(), unit.kind, unit.schedule),
        (
            InitializationUnitKey::TopLevelProperty(_)
                | InitializationUnitKey::ExtensionProperty(_),
            InitializationUnitKind::EagerTopLevel { .. },
            InitializationSchedule::EagerStartup,
        ) | (
            InitializationUnitKey::Object(_) | InitializationUnitKey::Companion(_),
            InitializationUnitKind::LazySingleton { .. },
            InitializationSchedule::LazyAccess,
        )
    );
    valid
        .then_some(())
        .ok_or(StrongInitializationUnitSemanticPlanBuildError::KindSchedule { unit: id })
}

fn validate_value_storage_key(
    unit: PersistentInitializationUnitId,
    semantic: &InitializationUnit,
    global: &crate::Global,
) -> Result<(), StrongInitializationUnitSemanticPlanBuildError> {
    let GlobalInit::Storage { identity, .. } = &global.init else {
        return Err(StrongInitializationUnitSemanticPlanBuildError::ValueStorageKey(unit));
    };
    let key = identity.identity_record().key();
    let valid = match semantic.identity.key() {
        InitializationUnitKey::TopLevelProperty(property) => {
            matches!(
                (key.owner(), key.role()),
                (
                    DefinitionOwner::Property(PropertyOwner::Property(actual)),
                    StorageRole::PropertyBacking | StorageRole::PropertyDelegate,
                ) if actual == *property
            )
        }
        InitializationUnitKey::ExtensionProperty(property) => {
            matches!(
                (key.owner(), key.role()),
                (
                    DefinitionOwner::Property(PropertyOwner::ExtensionProperty(actual)),
                    StorageRole::PropertyBacking | StorageRole::PropertyDelegate,
                ) if actual == *property
            )
        }
        InitializationUnitKey::Object(owner) | InitializationUnitKey::Companion(owner) => {
            key == &StaticStorageKey::singleton_published_root(*owner)
                && matches!(
                    key.owner(),
                    DefinitionOwner::Nominal(NominalOwner::Declaration(
                        NominalDeclarationOwner::Concrete(actual)
                    )) if actual == *owner
                )
        }
        InitializationUnitKey::GenericDelegatedExtensionApplication { .. } => false,
    };
    valid
        .then_some(())
        .ok_or(StrongInitializationUnitSemanticPlanBuildError::ValueStorageKey(unit))
}

fn validate_failure_root_key(
    unit: PersistentInitializationUnitId,
    global: &crate::Global,
) -> Result<(), StrongInitializationUnitSemanticPlanBuildError> {
    let GlobalInit::Storage { identity, .. } = &global.init else {
        return Err(StrongInitializationUnitSemanticPlanBuildError::FailureRootKey(unit));
    };
    (identity.identity_record().key() == &StaticStorageKey::initialization_failure_root(unit))
        .then_some(())
        .ok_or(StrongInitializationUnitSemanticPlanBuildError::FailureRootKey(unit))
}

fn validate_failure_shape(
    target: LirTargetProfile,
    unit: PersistentInitializationUnitId,
    storage: &StrongStaticStorageSemanticPlanV1,
) -> Result<(), StrongInitializationUnitSemanticPlanBuildError> {
    let pointer = target.pointer_layout(crate::PointerKind::Managed);
    if storage.byte_size() != pointer.size_bytes()
        || storage.allocation_extent() != pointer.size_bytes()
        || storage.required_alignment() != pointer.alignment_bytes()
        || storage.scan_program() != &RefScan::References(vec![0])
    {
        return Err(
            StrongInitializationUnitSemanticPlanBuildError::FailureRootShape {
                unit,
                storage: storage.storage(),
            },
        );
    }
    Ok(())
}

fn validate_function_reference(
    unit: PersistentInitializationUnitId,
    functions: &[crate::Function],
    reference: ManagedLocalFunctionRef,
    expected: PersistentCallableBodyId,
) -> Result<(), StrongInitializationUnitSemanticPlanBuildError> {
    let index = reference.declaration().into_u32() as usize;
    let Some(function) = functions.get(index) else {
        return Err(
            StrongInitializationUnitSemanticPlanBuildError::MissingFunctionReference {
                unit,
                index,
            },
        );
    };
    if function.gc_effect != GcEffect::Managed {
        return Err(
            StrongInitializationUnitSemanticPlanBuildError::FunctionReferenceEffect {
                unit,
                body: function.callable_body.id(),
                actual: function.gc_effect,
            },
        );
    }
    if function.callable_body.id() != expected {
        return Err(
            StrongInitializationUnitSemanticPlanBuildError::FunctionReferenceIdentity {
                unit,
                expected,
                actual: function.callable_body.id(),
            },
        );
    }
    let actual = functions
        .iter()
        .filter(|function| function.callable_body.id() == expected)
        .count();
    if actual != 1 {
        return Err(
            StrongInitializationUnitSemanticPlanBuildError::FunctionBodySet {
                unit,
                body: expected,
                actual,
            },
        );
    }
    Ok(())
}

pub(crate) fn generated_unit_body(
    unit: PersistentInitializationUnitId,
    role: InitializationCallableRole,
) -> Result<PersistentCallableBodyId, StrongInitializationUnitSemanticPlanBuildError> {
    let generated =
        PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
            unit,
            role,
        })
        .map_err(StrongInitializationUnitSemanticPlanBuildError::GeneratedCallableIdentity)?;
    PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::GeneratedCallable(generated),
    ))
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
    OdrUnit(PersistentInitializationUnitId),
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
pub use registrations::*;

#[cfg(test)]
mod tests;
