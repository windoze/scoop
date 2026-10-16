use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn push_sections(
    bytes: &mut Vec<u8>,
    module: &scoop_lir::Module,
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
    type_registrations: &StrongTypeRegistrationPlanSetV1,
    immortal_registrations: &StrongImmortalObjectRegistrationPlanSetV1,
    static_storage_registrations: &StrongStaticStorageRegistrationPlanSetV1,
    relocation_offset: u32,
    storage_is_zero_fill: bool,
) {
    for registration in registrations.registrations() {
        push_registration(bytes, *registration);
    }
    for registration in callable_registrations.registrations() {
        push_callable_registration(bytes, *registration);
    }
    for registration in type_registrations.registrations() {
        push_type_descriptor(bytes, registration);
    }
    for registration in type_registrations.registrations() {
        bytes.extend_from_slice(registration.semantic().diagnostic_name().as_bytes());
    }
    align_zero(bytes, 8);
    bytes.extend(std::iter::repeat_n(
        0,
        type_registrations.registrations().len() * usize::try_from(LAYOUT_SIZE).unwrap(),
    ));
    for registration in type_registrations.registrations() {
        push_type_registration(bytes, registration);
    }
    for registration in immortal_registrations.registrations() {
        push_immortal_object(bytes, module, *registration);
    }
    for registration in immortal_registrations.registrations() {
        push_immortal_registration(bytes, *registration);
    }
    if !static_storage_registrations.registrations().is_empty() {
        bytes.extend_from_slice(&[0; STATIC_EMPTY_SENTINELS_SIZE as usize]);
    }
    for registration in static_storage_registrations.registrations() {
        bytes.extend_from_slice(&[0; STATIC_LAYOUT_SIZE as usize]);
        push_scan_program(bytes, registration);
        if let StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table,
            ..
        } = registration.initial_artifacts()
        {
            bytes.extend_from_slice(registration.semantic().initial_state().initial_template());
            if matches!(
                relocation_table,
                scoop_lir::StaticStorageRelocationTableArtifactV1::Defined { .. }
            ) {
                for relocation in registration
                    .semantic()
                    .initial_state()
                    .immortal_relocations()
                {
                    push_u64(bytes, relocation.pointer_offset());
                    push_u64(bytes, 0);
                }
            }
        }
        push_static_storage_registration(bytes, registration);
    }
    if !storage_is_zero_fill {
        for registration in static_storage_registrations.registrations() {
            let initial = registration.semantic().initial_state().initial_template();
            bytes.extend_from_slice(initial);
            bytes.resize(
                bytes.len() + usize::try_from(registration.semantic().allocation_extent()).unwrap()
                    - initial.len(),
                0,
            );
        }
    }
    assert_eq!(bytes.len(), usize::try_from(relocation_offset).unwrap());
}
