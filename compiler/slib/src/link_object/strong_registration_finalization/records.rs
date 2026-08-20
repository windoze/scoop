use super::*;

pub(super) fn validate_final_safepoint(
    object: &[u8],
    checked_offset: u64,
    plan: scoop_lir::StrongSafepointRegistrationPlanV1,
    computed: &crate::link_object::VerifiedStrongSafepointFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let expected = expected_final_safepoint_record(plan, computed.stackmap().as_array());
    validate_final_record(
        object,
        checked_offset,
        &expected,
        StrongRegistrationPatchOwnerV1::Safepoint(plan.site()),
    )
}

pub(super) fn validate_final_callable(
    object: &[u8],
    checked_offset: u64,
    plan: scoop_lir::StrongCallableRegistrationPlanV1,
    computed: &crate::link_object::VerifiedStrongCallableFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let expected = expected_final_callable_record(plan, computed.body_definition().as_array());
    validate_final_record(
        object,
        checked_offset,
        &expected,
        StrongRegistrationPatchOwnerV1::Callable(plan.body()),
    )
}

pub(super) fn validate_final_type<D: Copy, C>(
    object: &[u8],
    checked_offset: u64,
    plan: &scoop_lir::StrongTypeRegistrationPlan<D, C>,
    computed: &crate::link_object::VerifiedStrongTypeFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let expected = expected_final_type_record(
        plan,
        computed.descriptor_definition().as_array(),
        computed.layout().as_array(),
    );
    validate_final_record(
        object,
        checked_offset,
        &expected,
        StrongRegistrationPatchOwnerV1::Type(plan.exact_type()),
    )
}

pub(super) fn validate_final_immortal_object(
    object: &[u8],
    checked_offset: u64,
    plan: scoop_lir::StrongImmortalObjectRegistrationPlanV1,
) -> Result<(), StrongRegistrationPatchError> {
    let expected = expected_final_immortal_object_record(plan);
    validate_final_record(
        object,
        checked_offset,
        &expected,
        StrongRegistrationPatchOwnerV1::ImmortalObject(plan.object()),
    )
}

pub(super) fn validate_final_static_storage(
    object: &[u8],
    verified: &crate::link_object::VerifiedStrongStaticStorageRegistrationV1,
    plan: &scoop_lir::StrongStaticStorageRegistrationPlanV1,
    computed: &crate::link_object::VerifiedStrongStaticStorageFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let expected = expected_final_static_storage_record(
        plan,
        verified.template_relocation().encoded_value(),
        verified.relocation_table_relocation().encoded_value(),
        computed.scan().as_array(),
        computed.layout().as_array(),
    );
    validate_final_record(
        object,
        verified.checked_offset(),
        &expected,
        StrongRegistrationPatchOwnerV1::StaticStorage(plan.semantic().storage()),
    )
}

pub(super) fn validate_final_initialization<I>(
    object: &[u8],
    checked_offset: u64,
    plan: &scoop_lir::StrongInitializationUnitRegistrationPlan<I>,
    computed: &crate::link_object::VerifiedStrongInitializationFingerprintV1,
) -> Result<(), StrongRegistrationPatchError> {
    let gateway_definition = computed.gateway_definition();
    let gateway = gateway_definition
        .as_ref()
        .map(|fingerprint| fingerprint.as_array());
    let expected = expected_final_initialization_record(plan, gateway);
    validate_final_record(
        object,
        checked_offset,
        &expected,
        StrongRegistrationPatchOwnerV1::InitializationUnit(plan.semantic().unit()),
    )
}

pub(super) fn validate_final_record(
    object: &[u8],
    checked_offset: u64,
    expected: &[u8],
    owner: StrongRegistrationPatchOwnerV1,
) -> Result<(), StrongRegistrationPatchError> {
    let start = usize::try_from(checked_offset)
        .map_err(|_| StrongRegistrationPatchError::RecordRange(owner))?;
    let end = start
        .checked_add(expected.len())
        .ok_or(StrongRegistrationPatchError::RecordRange(owner))?;
    let actual = object
        .get(start..end)
        .ok_or(StrongRegistrationPatchError::RecordRange(owner))?;
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(StrongRegistrationPatchError::FinalRecordMismatch {
            owner,
            offset: u16::try_from(offset).expect("descriptor offset fits u16"),
            expected: expected[offset],
            actual: actual[offset],
        });
    }
    Ok(())
}
