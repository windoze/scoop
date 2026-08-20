use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn push_sections(
    bytes: &mut Vec<u8>,
    module: &scoop_lir::Module,
    stackmap: &[u8],
    registrations: &StrongSafepointRegistrationPlanSetV1,
    callable_registrations: &StrongCallableRegistrationPlanSetV1,
    type_registrations: &StrongTypeRegistrationPlanSetV1,
    immortal_registrations: &StrongImmortalObjectRegistrationPlanSetV1,
    static_storage_registrations: &StrongStaticStorageRegistrationPlanSetV1,
    registration_offset: u32,
    relocation_offset: u32,
    storage_is_zero_fill: bool,
    corruption: Corruption,
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
    if matches!(corruption, Corruption::TypeDescriptorScalar) {
        let descriptor_offset = usize::try_from(
            type_descriptor_base(stackmap.len(), registrations, callable_registrations)
                - TEXT_SIZE
                - u64::try_from(stackmap.len()).unwrap(),
        )
        .unwrap()
            + usize::try_from(registration_offset).unwrap();
        bytes[descriptor_offset + 16] ^= 1;
    }
    for registration in type_registrations.registrations() {
        bytes.extend_from_slice(registration.semantic().diagnostic_name().as_bytes());
    }
    if matches!(corruption, Corruption::TypeDescriptorDiagnosticBytes) {
        let diagnostic_offset = usize::try_from(
            type_diagnostic_base(
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
            ) - TEXT_SIZE
                - u64::try_from(stackmap.len()).unwrap(),
        )
        .unwrap()
            + usize::try_from(registration_offset).unwrap();
        bytes[diagnostic_offset] ^= 1;
    }
    align_zero(bytes, 8);
    bytes.extend(std::iter::repeat_n(
        0,
        type_registrations.registrations().len() * usize::try_from(LAYOUT_SIZE).unwrap(),
    ));
    for registration in type_registrations.registrations() {
        push_type_registration(bytes, registration);
    }
    let first_immortal_object_offset = bytes.len();
    for registration in immortal_registrations.registrations() {
        push_immortal_object(bytes, module, *registration);
    }
    if matches!(corruption, Corruption::ImmortalObjectLength) {
        bytes[first_immortal_object_offset + 16..first_immortal_object_offset + 24]
            .copy_from_slice(&u64::MAX.to_le_bytes());
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
    if matches!(corruption, Corruption::RegistrationMagic) {
        bytes[usize::try_from(registration_offset).unwrap()] ^= 1;
    }
    if matches!(corruption, Corruption::CallableRegistrationMagic) {
        let offset = u64::from(registration_offset)
            + u64::try_from(registrations.registrations().len()).unwrap()
                * SAFEPOINT_REGISTRATION_SIZE;
        bytes[usize::try_from(offset).unwrap()] ^= 1;
    }
    if matches!(corruption, Corruption::TypeRegistrationMagic) {
        let offset = type_registration_base(
            stackmap.len(),
            registrations,
            callable_registrations,
            type_registrations,
        );
        bytes[usize::try_from(
            u64::from(registration_offset) - TEXT_SIZE - u64::try_from(stackmap.len()).unwrap()
                + offset,
        )
        .unwrap()] ^= 1;
    }
    if matches!(corruption, Corruption::ImmortalRegistrationMagic) {
        let offset = immortal_registration_base(
            stackmap.len(),
            registrations,
            callable_registrations,
            type_registrations,
            immortal_registrations,
        );
        bytes[usize::try_from(
            u64::from(registration_offset) - TEXT_SIZE - u64::try_from(stackmap.len()).unwrap()
                + offset,
        )
        .unwrap()] ^= 1;
    }
    if matches!(
        corruption,
        Corruption::StaticRegistrationMagic | Corruption::StaticScanProgram
    ) {
        let registration = &static_storage_registrations.registrations()[0];
        let virtual_offset = if matches!(corruption, Corruption::StaticRegistrationMagic) {
            static_registration_start(
                registration,
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            )
        } else {
            static_scan_start(
                registration,
                stackmap.len(),
                registrations,
                callable_registrations,
                type_registrations,
                immortal_registrations,
                static_storage_registrations,
            )
        };
        let file_offset = u64::from(registration_offset) + virtual_offset
            - TEXT_SIZE
            - u64::try_from(stackmap.len()).unwrap();
        bytes[usize::try_from(file_offset).unwrap()] ^= 1;
    }
}
