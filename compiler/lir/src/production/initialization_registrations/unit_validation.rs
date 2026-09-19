//! Semantic validation of initialization storage and callables.

use super::*;

pub(super) fn require_storage<'storage>(
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

pub(super) fn require_zeroed(
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

pub(super) fn validate_unit_kind_and_schedule(
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

pub(super) fn validate_value_storage_key(
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

pub(super) fn validate_failure_root_key(
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

pub(super) fn validate_failure_shape(
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

pub(super) fn validate_function_reference(
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
