use super::*;

pub(super) fn registration_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    registrations: &StrongSafepointRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.definition_plan() == definition)
        .unwrap();
    TEXT_SIZE
        + u64::try_from(stackmap_size).unwrap()
        + u64::try_from(index).unwrap() * SAFEPOINT_REGISTRATION_SIZE
}

pub(super) fn callable_registration_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    registrations: &StrongCallableRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.definition_plan() == definition)
        .unwrap();
    TEXT_SIZE
        + u64::try_from(stackmap_size).unwrap()
        + u64::try_from(safepoints.registrations().len()).unwrap() * SAFEPOINT_REGISTRATION_SIZE
        + u64::try_from(index).unwrap() * CALLABLE_REGISTRATION_SIZE
}

pub(super) fn type_descriptor_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
) -> u64 {
    TEXT_SIZE
        + u64::try_from(stackmap_size).unwrap()
        + u64::try_from(safepoints.registrations().len()).unwrap() * SAFEPOINT_REGISTRATION_SIZE
        + u64::try_from(callables.registrations().len()).unwrap() * CALLABLE_REGISTRATION_SIZE
}

pub(super) fn type_descriptor_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.descriptor_definition_plan() == definition)
        .unwrap();
    type_descriptor_base(stackmap_size, safepoints, callables)
        + u64::try_from(index).unwrap() * TYPE_DESCRIPTOR_SIZE
}

pub(super) fn layout_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    type_diagnostic_base(stackmap_size, safepoints, callables, registrations)
        + type_diagnostic_region_size(registrations)
}

pub(super) fn type_diagnostic_region_size(registrations: &StrongTypeRegistrationPlanSetV1) -> u64 {
    let size = registrations
        .registrations()
        .iter()
        .map(|registration| u64::try_from(registration.semantic().diagnostic_name().len()).unwrap())
        .sum::<u64>();
    (size + 7) & !7
}

pub(super) fn type_diagnostic_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    type_descriptor_base(stackmap_size, safepoints, callables)
        + u64::try_from(registrations.registrations().len()).unwrap() * TYPE_DESCRIPTOR_SIZE
}

pub(super) fn type_diagnostic_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.descriptor_definition_plan() == definition)
        .unwrap();
    type_diagnostic_base(stackmap_size, safepoints, callables, registrations)
        + registrations.registrations()[..index]
            .iter()
            .map(|registration| {
                u64::try_from(registration.semantic().diagnostic_name().len()).unwrap()
            })
            .sum::<u64>()
}

pub(super) fn type_diagnostic_end(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    let registration = registrations
        .registrations()
        .iter()
        .find(|registration| registration.descriptor_definition_plan() == definition)
        .unwrap();
    type_diagnostic_start(
        definition,
        stackmap_size,
        safepoints,
        callables,
        registrations,
    ) + u64::try_from(registration.semantic().diagnostic_name().len()).unwrap()
}

pub(super) fn layout_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.layout_definition_plan() == definition)
        .unwrap();
    layout_base(stackmap_size, safepoints, callables, registrations)
        + u64::try_from(index).unwrap() * LAYOUT_SIZE
}

pub(super) fn type_registration_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    layout_base(stackmap_size, safepoints, callables, registrations)
        + u64::try_from(registrations.registrations().len()).unwrap() * LAYOUT_SIZE
}

pub(super) fn type_registration_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.definition_plan() == definition)
        .unwrap();
    type_registration_base(stackmap_size, safepoints, callables, registrations)
        + u64::try_from(index).unwrap() * TYPE_REGISTRATION_SIZE
}

pub(super) fn immortal_object_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
) -> u64 {
    type_registration_base(stackmap_size, safepoints, callables, types)
        + u64::try_from(types.registrations().len()).unwrap() * TYPE_REGISTRATION_SIZE
}

pub(super) fn immortal_object_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    registrations: &StrongImmortalObjectRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.object_definition_plan() == definition)
        .unwrap();
    immortal_object_base(stackmap_size, safepoints, callables, types)
        + registrations.registrations()[..index]
            .iter()
            .map(|registration| registration.object_size())
            .sum::<u64>()
}

pub(super) fn immortal_registration_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    registrations: &StrongImmortalObjectRegistrationPlanSetV1,
) -> u64 {
    immortal_object_base(stackmap_size, safepoints, callables, types)
        + registrations
            .registrations()
            .iter()
            .map(|registration| registration.object_size())
            .sum::<u64>()
}

pub(super) fn immortal_registration_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    registrations: &StrongImmortalObjectRegistrationPlanSetV1,
) -> u64 {
    let index = registrations
        .registrations()
        .iter()
        .position(|registration| registration.registration_definition_plan() == definition)
        .unwrap();
    immortal_registration_base(stackmap_size, safepoints, callables, types, registrations)
        + u64::try_from(index).unwrap() * IMMORTAL_REGISTRATION_SIZE
}

pub(super) fn static_sentinel_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
) -> u64 {
    immortal_registration_base(stackmap_size, safepoints, callables, types, immortals)
        + u64::try_from(immortals.registrations().len()).unwrap() * IMMORTAL_REGISTRATION_SIZE
}

pub(super) fn static_readonly_base(
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_sentinel_base(stackmap_size, safepoints, callables, types, immortals)
        + u64::from(!statics.registrations().is_empty()) * STATIC_EMPTY_SENTINELS_SIZE
}

pub(super) fn static_scan_size(registration: &StrongStaticStorageRegistrationPlanV1) -> u64 {
    match registration.semantic().scan_program() {
        scoop_lir::RefScan::None => 8,
        scoop_lir::RefScan::References(offsets) => (u64::try_from(offsets.len()).unwrap() + 1) * 8,
        scoop_lir::RefScan::Sequence(_) | scoop_lir::RefScan::Array { .. } => {
            panic!("fixture cannot contain non-value scans")
        }
    }
}

pub(super) fn static_template_size(registration: &StrongStaticStorageRegistrationPlanV1) -> u64 {
    match registration.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit => 0,
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue { .. } => u64::try_from(
            registration
                .semantic()
                .initial_state()
                .initial_template()
                .len(),
        )
        .unwrap(),
    }
}

pub(super) fn static_relocation_size(registration: &StrongStaticStorageRegistrationPlanV1) -> u64 {
    match registration.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: scoop_lir::StaticStorageRelocationTableArtifactV1::Defined { .. },
            ..
        } => {
            u64::try_from(
                registration
                    .semantic()
                    .initial_state()
                    .immortal_relocations()
                    .len(),
            )
            .unwrap()
                * 16
        }
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit
        | StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: scoop_lir::StaticStorageRelocationTableArtifactV1::SharedEmptySentinel,
            ..
        } => 0,
    }
}

pub(super) fn static_readonly_group_size(
    registration: &StrongStaticStorageRegistrationPlanV1,
) -> u64 {
    STATIC_LAYOUT_SIZE
        + static_scan_size(registration)
        + static_template_size(registration)
        + static_relocation_size(registration)
        + STATIC_STORAGE_REGISTRATION_SIZE
}

pub(super) fn static_readonly_group_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    let index = statics
        .registrations()
        .iter()
        .position(|candidate| candidate.semantic().storage() == registration.semantic().storage())
        .unwrap();
    static_readonly_base(
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + statics.registrations()[..index]
        .iter()
        .map(static_readonly_group_size)
        .sum::<u64>()
}

pub(super) fn static_layout_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_readonly_group_start(
        registration,
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    )
}

pub(super) fn static_scan_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_layout_start(
        registration,
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + STATIC_LAYOUT_SIZE
}

pub(super) fn static_template_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_scan_start(
        registration,
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + static_scan_size(registration)
}

pub(super) fn static_relocation_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_template_start(
        registration,
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + static_template_size(registration)
}

pub(super) fn static_registration_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    static_relocation_start(
        registration,
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + static_relocation_size(registration)
}

pub(super) fn static_template_target_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    match registration.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit => {
            static_sentinel_base(stackmap_size, safepoints, callables, types, immortals)
        }
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue { .. } => {
            static_template_start(
                registration,
                stackmap_size,
                safepoints,
                callables,
                types,
                immortals,
                statics,
            )
        }
    }
}

pub(super) fn static_relocation_target_start(
    registration: &StrongStaticStorageRegistrationPlanV1,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    match registration.initial_artifacts() {
        StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: scoop_lir::StaticStorageRelocationTableArtifactV1::Defined { .. },
            ..
        } => static_relocation_start(
            registration,
            stackmap_size,
            safepoints,
            callables,
            types,
            immortals,
            statics,
        ),
        StrongStaticStorageInitialArtifactPlanV1::ZeroedForRuntimeUnit
        | StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table: scoop_lir::StaticStorageRelocationTableArtifactV1::SharedEmptySentinel,
            ..
        } => static_sentinel_base(stackmap_size, safepoints, callables, types, immortals) + 8,
    }
}

pub(super) fn static_registration_for_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongStaticStorageRegistrationPlanSetV1,
    select: impl Fn(&StrongStaticStorageRegistrationPlanV1) -> ObjectDefinitionPlanId,
) -> &StrongStaticStorageRegistrationPlanV1 {
    registrations
        .registrations()
        .iter()
        .find(|registration| select(registration) == definition)
        .unwrap()
}

macro_rules! static_start_for_definition {
    ($name:ident, $select:ident, $start:ident) => {
        pub(super) fn $name(
            definition: ObjectDefinitionPlanId,
            stackmap_size: usize,
            safepoints: &StrongSafepointRegistrationPlanSetV1,
            callables: &StrongCallableRegistrationPlanSetV1,
            types: &StrongTypeRegistrationPlanSetV1,
            immortals: &StrongImmortalObjectRegistrationPlanSetV1,
            statics: &StrongStaticStorageRegistrationPlanSetV1,
        ) -> u64 {
            let registration =
                static_registration_for_definition(definition, statics, |registration| {
                    registration.$select()
                });
            $start(
                registration,
                stackmap_size,
                safepoints,
                callables,
                types,
                immortals,
                statics,
            )
        }
    };
}

static_start_for_definition!(
    static_template_start_for_definition,
    storage_definition_plan,
    static_template_start
);
static_start_for_definition!(
    static_relocation_start_for_definition,
    storage_definition_plan,
    static_relocation_start
);
static_start_for_definition!(
    static_registration_start_for_definition,
    registration_definition_plan,
    static_registration_start
);
static_start_for_definition!(
    static_layout_start_for_definition,
    layout_definition_plan,
    static_layout_start
);
static_start_for_definition!(
    static_scan_start_for_definition,
    scan_definition_plan,
    static_scan_start
);

pub(super) fn static_storage_start(
    definition: ObjectDefinitionPlanId,
    stackmap_size: usize,
    safepoints: &StrongSafepointRegistrationPlanSetV1,
    callables: &StrongCallableRegistrationPlanSetV1,
    types: &StrongTypeRegistrationPlanSetV1,
    immortals: &StrongImmortalObjectRegistrationPlanSetV1,
    statics: &StrongStaticStorageRegistrationPlanSetV1,
) -> u64 {
    let index = statics
        .registrations()
        .iter()
        .position(|registration| registration.storage_definition_plan() == definition)
        .unwrap();
    static_readonly_base(
        stackmap_size,
        safepoints,
        callables,
        types,
        immortals,
        statics,
    ) + statics
        .registrations()
        .iter()
        .map(static_readonly_group_size)
        .sum::<u64>()
        + statics.registrations()[..index]
            .iter()
            .map(|registration| registration.semantic().allocation_extent())
            .sum::<u64>()
}
