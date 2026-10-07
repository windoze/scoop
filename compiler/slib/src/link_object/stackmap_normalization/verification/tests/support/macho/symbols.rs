use super::*;

pub(super) fn symbol_location(
    role: PlannedStrongObjectSymbolRoleV1,
    stackmap_size: usize,
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
    type_registrations: &StrongTypeRegistrationPlanSetV1,
    immortal_registrations: &StrongImmortalObjectRegistrationPlanSetV1,
    static_storage_registrations: &StrongStaticStorageRegistrationPlanSetV1,
) -> (u8, u64) {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::CallableBody,
            ..
        } => (1, 0),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            atom_role: DefinitionAtomRole::Primary,
            definition,
            ..
        } if is_callable_body_definition(definition, callable_registrations) => (1, 0),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            atom_role: DefinitionAtomRole::Primary,
            definition,
            ..
        } if is_callable_body_definition(definition, callable_registrations) => (1, TEXT_SIZE),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Stackmap | DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_callable_body_definition(definition, callable_registrations) => (2, TEXT_SIZE),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Stackmap | DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_callable_body_definition(definition, callable_registrations) => {
            (2, TEXT_SIZE + u64::try_from(stackmap_size).unwrap())
        }
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::SafepointRegistration,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_safepoint_registration_definition(definition, registrations) => (
            3,
            registration_start(definition, stackmap_size, registrations),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_safepoint_registration_definition(definition, registrations) => (
            3,
            registration_start(definition, stackmap_size, registrations)
                + SAFEPOINT_REGISTRATION_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::CallableRegistration,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_callable_registration_definition(definition, callable_registrations) => (
            3,
            callable_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_callable_registration_definition(definition, callable_registrations) => (
            3,
            callable_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
            ) + CALLABLE_REGISTRATION_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::TypeDescriptor,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_type_descriptor_definition(definition, type_registrations) => (
            3,
            type_descriptor_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_type_descriptor_definition(definition, type_registrations) => (
            3,
            type_descriptor_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ) + TYPE_DESCRIPTOR_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_type_descriptor_definition(definition, type_registrations) => (
            3,
            type_diagnostic_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_type_descriptor_definition(definition, type_registrations) => (
            3,
            type_diagnostic_end(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::Layout,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_layout_definition(definition, type_registrations) => (
            3,
            layout_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_layout_definition(definition, type_registrations) => (
            3,
            layout_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ) + LAYOUT_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::TypeRegistration,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_type_registration_definition(definition, type_registrations) => (
            3,
            type_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_type_registration_definition(definition, type_registrations) => (
            3,
            type_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
            ) + TYPE_REGISTRATION_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::ImmortalObject,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_immortal_object_definition(definition, immortal_registrations) => (
            3,
            immortal_object_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_immortal_object_definition(definition, immortal_registrations) => (
            3,
            immortal_object_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            ) + immortal_registrations
                .registrations()
                .iter()
                .find(|registration| registration.object_definition_plan() == definition)
                .unwrap()
                .object_size(),
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::ImmortalRegistration,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_immortal_registration_definition(definition, immortal_registrations) => (
            3,
            immortal_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_immortal_registration_definition(definition, immortal_registrations) => (
            3,
            immortal_registration_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
            ) + IMMORTAL_REGISTRATION_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::StaticStorage,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            4,
            static_storage_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            4,
            static_storage_start(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + static_storage_registrations
                .registrations()
                .iter()
                .find(|registration| registration.storage_definition_plan() == definition)
                .unwrap()
                .semantic()
                .allocation_extent(),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            3,
            static_template_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::AddressTakenConstant,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            3,
            static_template_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + static_storage_registrations
                .registrations()
                .iter()
                .find(|registration| registration.storage_definition_plan() == definition)
                .map(static_template_size)
                .unwrap(),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::RuntimeRecord,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            3,
            static_relocation_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::RuntimeRecord,
            ..
        } if is_static_storage_definition(definition, static_storage_registrations) => (
            3,
            static_relocation_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + static_storage_registrations
                .registrations()
                .iter()
                .find(|registration| registration.storage_definition_plan() == definition)
                .map(static_relocation_size)
                .unwrap(),
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::RootRegistration,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_registration_definition(definition, static_storage_registrations) => (
            3,
            static_registration_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_registration_definition(definition, static_storage_registrations) => (
            3,
            static_registration_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + STATIC_STORAGE_REGISTRATION_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::Layout,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_layout_definition(definition, static_storage_registrations) => (
            3,
            static_layout_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_layout_definition(definition, static_storage_registrations) => (
            3,
            static_layout_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + STATIC_LAYOUT_SIZE,
        ),
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
            definition_role: StrongDefinitionRole::ScanProgram,
            definition,
            ..
        }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_scan_definition(definition, static_storage_registrations) => (
            3,
            static_scan_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ),
        ),
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd {
            definition,
            atom_role: DefinitionAtomRole::Primary,
            ..
        } if is_static_scan_definition(definition, static_storage_registrations) => (
            3,
            static_scan_start_for_definition(
                definition,
                stackmap_size,
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            ) + static_storage_registrations
                .registrations()
                .iter()
                .find(|registration| registration.scan_definition_plan() == definition)
                .map(static_scan_size)
                .unwrap(),
        ),
        _ => unreachable!("fixture has only primary and stackmap atoms"),
    }
}

pub(super) fn is_callable_body_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongCallableRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.body_definition_plan() == definition)
}

pub(super) fn is_safepoint_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongSafepointRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.definition_plan() == definition)
}

pub(super) fn is_callable_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongCallableRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.definition_plan() == definition)
}

pub(super) fn is_type_descriptor_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.descriptor_definition_plan() == definition)
}

pub(super) fn is_layout_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.layout_definition_plan() == definition)
}

pub(super) fn is_type_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongTypeRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.definition_plan() == definition)
}

pub(super) fn is_immortal_object_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongImmortalObjectRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.object_definition_plan() == definition)
}

pub(super) fn is_immortal_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongImmortalObjectRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.registration_definition_plan() == definition)
}

pub(super) fn is_static_storage_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongStaticStorageRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.storage_definition_plan() == definition)
}

pub(super) fn is_static_registration_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongStaticStorageRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.registration_definition_plan() == definition)
}

pub(super) fn is_static_layout_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongStaticStorageRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.layout_definition_plan() == definition)
}

pub(super) fn is_static_scan_definition(
    definition: ObjectDefinitionPlanId,
    registrations: &StrongStaticStorageRegistrationPlanSetV1,
) -> bool {
    registrations
        .registrations()
        .iter()
        .any(|registration| registration.scan_definition_plan() == definition)
}
