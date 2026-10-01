use scoop_lir::StrongTypeRegistrationPlan;

use super::StrongTypeRegistrationValidationError;

const DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5054_5950;
pub(in crate::link_object) const ABI_VERSION: u32 = 3;
pub(in crate::link_object) const DESCRIPTOR_SIZE: usize = 240;

pub(super) fn validate_record_bytes<D: Copy, C>(
    object: &[u8],
    file_start: u64,
    plan: &StrongTypeRegistrationPlan<D, C>,
) -> Result<(), StrongTypeRegistrationValidationError> {
    let start = usize::try_from(file_start).map_err(|_| {
        StrongTypeRegistrationValidationError::RecordRangeOverflow(plan.exact_type())
    })?;
    let end = start.checked_add(DESCRIPTOR_SIZE).ok_or(
        StrongTypeRegistrationValidationError::RecordRangeOverflow(plan.exact_type()),
    )?;
    let actual = object.get(start..end).ok_or(
        StrongTypeRegistrationValidationError::RecordRangeOverflow(plan.exact_type()),
    )?;
    let expected = expected_record(plan);
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| *actual != expected)
    {
        return Err(StrongTypeRegistrationValidationError::RecordByteMismatch {
            exact_type: plan.exact_type(),
            offset_within_atom: u16::try_from(offset).expect("descriptor offset fits u16"),
            expected: expected[offset],
            actual: actual[offset],
        });
    }
    Ok(())
}

pub(super) fn expected_record<D: Copy, C>(
    plan: &StrongTypeRegistrationPlan<D, C>,
) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = [0; DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, ABI_VERSION);
    write_u32(&mut bytes, 12, DESCRIPTOR_SIZE as u32);
    bytes[16..152].copy_from_slice(
        &crate::link_object::registration_identity::provisional_registration_identity(
            plan.exact_type().as_array(),
            plan.definition_owner(),
        ),
    );
    write_u64(&mut bytes, 152, plan.runtime_type().get());
    bytes
}

pub(in crate::link_object) fn expected_final_record<D: Copy, C>(
    plan: &StrongTypeRegistrationPlan<D, C>,
    registration_definition: &[u8; 32],
    descriptor_definition: &[u8; 32],
    layout: &[u8; 32],
) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = expected_record(plan);
    bytes[120..152].copy_from_slice(registration_definition);
    bytes[176..208].copy_from_slice(descriptor_definition);
    bytes[208..240].copy_from_slice(layout);
    bytes
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
