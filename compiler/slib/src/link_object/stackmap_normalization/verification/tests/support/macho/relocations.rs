use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn push_relocations(
    bytes: &mut Vec<u8>,
    symbols: &PlannedMemberStrongObjectSymbolsV1,
    stackmap: &[u8],
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
    type_registrations: &StrongTypeRegistrationPlanSetV1,
    immortal_registrations: &StrongImmortalObjectRegistrationPlanSetV1,
    static_storage_registrations: &StrongStaticStorageRegistrationPlanSetV1,
    local_symbol_count: usize,
    static_local_symbol_count: usize,
    registration_relocation_count: u32,
    corruption: Corruption,
) {
    let planned_index = |index: usize| u32::try_from(local_symbol_count + index).unwrap();
    let target = symbols
        .symbols()
        .iter()
        .position(|symbol| {
            matches!(
                symbol.role(),
                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                    definition_role: StrongDefinitionRole::CallableBody,
                    ..
                }
            )
        })
        .unwrap();
    push_u32(bytes, 8);
    push_u32(
        bytes,
        planned_index(target)
            | 1 << 24
            | 2 << 25
            | 1 << 27
            | u32::from(macho::ARM64_RELOC_BRANCH26) << 28,
    );
    push_u32(bytes, 16);
    push_u32(bytes, planned_index(target) | 3 << 25 | 1 << 27);
    if registration_relocation_count != 0 {
        if matches!(corruption, Corruption::RelocatedRegistration) {
            push_u32(bytes, 56);
            push_u32(bytes, planned_index(target) | 3 << 25 | 1 << 27);
        }
        for (index, _) in callable_registrations.registrations().iter().enumerate() {
            let safepoint_bytes = u32::try_from(registrations.registrations().len()).unwrap()
                * u32::try_from(SAFEPOINT_REGISTRATION_SIZE).unwrap();
            let callable_bytes =
                u32::try_from(index).unwrap() * u32::try_from(CALLABLE_REGISTRATION_SIZE).unwrap();
            push_u32(bytes, safepoint_bytes + callable_bytes + 152);
            let relocation_target =
                if matches!(corruption, Corruption::CallableEntryRelocationTarget) {
                    symbols
                        .symbols()
                        .iter()
                        .position(|symbol| {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::CallableRegistration,
                                    ..
                                }
                            )
                        })
                        .unwrap()
                } else {
                    target
                };
            push_u32(bytes, planned_index(relocation_target) | 3 << 25 | 1 << 27);
        }
        for (index, registration) in type_registrations.registrations().iter().enumerate() {
            let descriptor_start = type_descriptor_start(
                registration.descriptor_definition_plan(),
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
            );
            push_u32(
                bytes,
                u32::try_from(
                    descriptor_start - TEXT_SIZE - u64::try_from(stackmap.len()).unwrap() + 112,
                )
                .unwrap(),
            );
            let diagnostic_target = if matches!(
                corruption,
                Corruption::TypeDescriptorDiagnosticRelocationTarget
            ) && index == 0
            {
                symbols
                    .symbols()
                    .iter()
                    .position(|symbol| {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition_role: StrongDefinitionRole::TypeDescriptor,
                                definition,
                                ..
                            } if definition == registration.descriptor_definition_plan()
                        )
                    })
                    .map(planned_index)
                    .unwrap()
            } else {
                u32::try_from(static_local_symbol_count + index).unwrap()
            };
            push_u32(bytes, diagnostic_target | 3 << 25 | 1 << 27);
        }
        for (index, registration) in type_registrations.registrations().iter().enumerate() {
            let type_start = type_registration_start(
                registration.definition_plan(),
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
            );
            push_u32(
                bytes,
                u32::try_from(
                    type_start - TEXT_SIZE - u64::try_from(stackmap.len()).unwrap() + 136,
                )
                .unwrap(),
            );
            let descriptor_target =
                if matches!(corruption, Corruption::TypeDescriptorRelocationTarget) && index == 0 {
                    symbols
                        .symbols()
                        .iter()
                        .position(|symbol| {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::TypeRegistration,
                                    ..
                                }
                            )
                        })
                        .unwrap()
                } else {
                    symbols
                        .symbols()
                        .iter()
                        .position(|symbol| {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::TypeDescriptor,
                                    definition,
                                    ..
                                } if definition == registration.descriptor_definition_plan()
                            )
                        })
                        .unwrap()
                };
            push_u32(bytes, planned_index(descriptor_target) | 3 << 25 | 1 << 27);
        }
        for (index, registration) in immortal_registrations.registrations().iter().enumerate() {
            let object_start = immortal_object_start(
                registration.object_definition_plan(),
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            );
            let record_start = immortal_registration_start(
                registration.registration_definition_plan(),
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            );
            let section_base = TEXT_SIZE + u64::try_from(stackmap.len()).unwrap();
            let descriptor_target = if matches!(
                corruption,
                Corruption::ImmortalObjectDescriptorRelocationTarget
            ) && index == 0
            {
                symbols
                    .symbols()
                    .iter()
                    .position(|symbol| {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition_role: StrongDefinitionRole::ImmortalRegistration,
                                ..
                            }
                        )
                    })
                    .unwrap()
            } else if matches!(
                registration.semantic().type_registration_ref(),
                scoop_lir::ImmortalObjectTypeRegistrationRefV1::DependencyExternal { .. }
            ) {
                symbols.symbols().len()
            } else {
                symbols
                    .symbols()
                    .iter()
                    .position(|symbol| {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition_role: StrongDefinitionRole::TypeDescriptor,
                                ..
                            }
                        )
                    })
                    .unwrap()
            };
            push_u32(bytes, u32::try_from(object_start - section_base).unwrap());
            push_u32(bytes, planned_index(descriptor_target) | 3 << 25 | 1 << 27);

            let object_target =
                if matches!(corruption, Corruption::ImmortalObjectRelocationTarget) && index == 0 {
                    symbols
                        .symbols()
                        .iter()
                        .position(|symbol| {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::ImmortalRegistration,
                                    ..
                                }
                            )
                        })
                        .unwrap()
                } else {
                    symbols
                        .symbols()
                        .iter()
                        .position(|symbol| {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::ImmortalObject,
                                    definition,
                                    ..
                                } if definition == registration.object_definition_plan()
                            )
                        })
                        .unwrap()
                };
            push_u32(
                bytes,
                u32::try_from(record_start - section_base + 120).unwrap(),
            );
            push_u32(bytes, planned_index(object_target) | 3 << 25 | 1 << 27);

            let type_target = if matches!(
                registration.semantic().type_registration_ref(),
                scoop_lir::ImmortalObjectTypeRegistrationRefV1::DependencyExternal { .. }
            ) {
                symbols.symbols().len() + 1
            } else if matches!(
                corruption,
                Corruption::ImmortalTypeRegistrationRelocationTarget
            ) && index == 0
            {
                object_target
            } else {
                symbols
                    .symbols()
                    .iter()
                    .position(|symbol| {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition_role: StrongDefinitionRole::TypeRegistration,
                                ..
                            }
                        )
                    })
                    .unwrap()
            };
            push_u32(
                bytes,
                u32::try_from(record_start - section_base + 144).unwrap(),
            );
            push_u32(bytes, planned_index(type_target) | 3 << 25 | 1 << 27);
        }
        for (index, registration) in static_storage_registrations
            .registrations()
            .iter()
            .enumerate()
        {
            let section_base = TEXT_SIZE + u64::try_from(stackmap.len()).unwrap();
            let record_start = static_registration_start(
                registration,
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            );
            let storage_target = symbols
                .symbols()
                .iter()
                .position(|symbol| {
                    if matches!(corruption, Corruption::StaticStorageRelocationTarget) && index == 0
                    {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition,
                                definition_role: StrongDefinitionRole::RootRegistration,
                                ..
                            } if definition == registration.registration_definition_plan()
                        )
                    } else {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition,
                                definition_role: StrongDefinitionRole::StaticStorage,
                                ..
                            } if definition == registration.storage_definition_plan()
                        )
                    }
                })
                .unwrap();
            let scan_target = symbols
                .symbols()
                .iter()
                .position(|symbol| {
                    matches!(
                        symbol.role(),
                        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                            definition,
                            definition_role: StrongDefinitionRole::ScanProgram,
                            ..
                        } if definition == registration.scan_definition_plan()
                    )
                })
                .unwrap();
            for (offset, target) in [
                (128, planned_index(storage_target)),
                (160, planned_index(scan_target)),
                (232, u32::try_from(index * 2).unwrap()),
                (
                    248,
                    u32::try_from(
                        if matches!(corruption, Corruption::StaticSentinelCollision) {
                            index * 2
                        } else {
                            index * 2 + 1
                        },
                    )
                    .unwrap(),
                ),
            ] {
                push_u32(
                    bytes,
                    u32::try_from(record_start - section_base + offset).unwrap(),
                );
                push_u32(bytes, target | 3 << 25 | 1 << 27);
            }
            let relocation_start = static_relocation_start(
                registration,
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            );
            for (relocation_index, relocation) in registration
                .semantic()
                .initial_state()
                .immortal_relocations()
                .iter()
                .enumerate()
            {
                let immortal_registration_definition = immortal_registrations
                    .registrations()
                    .iter()
                    .find(|immortal| immortal.object() == relocation.target())
                    .unwrap()
                    .registration_definition_plan();
                let immortal_target = symbols
                    .symbols()
                    .iter()
                    .position(|symbol| {
                        if matches!(corruption, Corruption::StaticInitialTableRelocationTarget)
                            && index == 0
                            && relocation_index == 0
                        {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition_role: StrongDefinitionRole::ImmortalObject,
                                    ..
                                }
                            )
                        } else {
                            matches!(
                                symbol.role(),
                                PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                    definition,
                                    definition_role: StrongDefinitionRole::ImmortalRegistration,
                                    ..
                                } if definition == immortal_registration_definition
                            )
                        }
                    })
                    .unwrap();
                push_u32(
                    bytes,
                    u32::try_from(
                        relocation_start - section_base
                            + u64::try_from(relocation_index).unwrap() * 16
                            + 8,
                    )
                    .unwrap(),
                );
                push_u32(bytes, planned_index(immortal_target) | 3 << 25 | 1 << 27);
            }
        }
    }
    let mut storage_base = 0_u64;
    for (static_index, registration) in static_storage_registrations
        .registrations()
        .iter()
        .enumerate()
    {
        for relocation in registration
            .semantic()
            .initial_state()
            .immortal_relocations()
        {
            let immortal_object_definition = immortal_registrations
                .registrations()
                .iter()
                .find(|immortal| immortal.object() == relocation.target())
                .unwrap()
                .object_definition_plan();
            let immortal_target = symbols
                .symbols()
                .iter()
                .position(|symbol| {
                    if matches!(corruption, Corruption::StaticInitialStorageRelocationTarget)
                        && static_index == 0
                    {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition_role: StrongDefinitionRole::ImmortalRegistration,
                                ..
                            }
                        )
                    } else {
                        matches!(
                            symbol.role(),
                            PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                                definition,
                                definition_role: StrongDefinitionRole::ImmortalObject,
                                ..
                            } if definition == immortal_object_definition
                        )
                    }
                })
                .unwrap();
            push_u32(
                bytes,
                u32::try_from(storage_base + relocation.pointer_offset()).unwrap(),
            );
            push_u32(bytes, planned_index(immortal_target) | 3 << 25 | 1 << 27);
        }
        storage_base += registration.semantic().allocation_extent();
    }
}
