use scoop_lir::StrongCallableRegistrationPlanV1;

use super::StrongCallableRegistrationValidationError;

const DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5043_414c;
const ABI_VERSION: u32 = 1;
const DEFINITION_FINGERPRINT_OFFSET: usize = 120;
const BODY_DEFINITION_FINGERPRINT_OFFSET: usize = 152;
const DIGEST_WIDTH: usize = 32;
pub(super) const DESCRIPTOR_SIZE: usize = 192;

pub(super) fn validate_record_bytes(
    object: &[u8],
    file_start: u64,
    plan: StrongCallableRegistrationPlanV1,
) -> Result<(), StrongCallableRegistrationValidationError> {
    let start = usize::try_from(file_start)
        .map_err(|_| StrongCallableRegistrationValidationError::RecordRangeOverflow(plan.body()))?;
    let end = start
        .checked_add(DESCRIPTOR_SIZE)
        .ok_or(StrongCallableRegistrationValidationError::RecordRangeOverflow(plan.body()))?;
    let actual = object
        .get(start..end)
        .ok_or(StrongCallableRegistrationValidationError::RecordRangeOverflow(plan.body()))?;
    let expected = expected_record(plan);
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| *actual != expected)
    {
        return Err(
            StrongCallableRegistrationValidationError::RecordByteMismatch {
                body: plan.body(),
                offset_within_atom: u16::try_from(offset).expect("descriptor offset fits u16"),
                expected: expected[offset],
                actual: actual[offset],
            },
        );
    }
    Ok(())
}

pub(in crate::link_object) fn expected_record(
    plan: StrongCallableRegistrationPlanV1,
) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = [0; DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, ABI_VERSION);
    write_u32(&mut bytes, 12, DESCRIPTOR_SIZE as u32);
    bytes[16..152].copy_from_slice(
        &crate::link_object::registration_identity::provisional_registration_identity(
            plan.body().as_array(),
            plan.definition_owner(),
        ),
    );
    bytes
}

pub(in crate::link_object) fn expected_final_record(
    plan: StrongCallableRegistrationPlanV1,
    registration: &[u8; DIGEST_WIDTH],
    body_definition: &[u8; DIGEST_WIDTH],
) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = expected_record(plan);
    bytes[DEFINITION_FINGERPRINT_OFFSET..DEFINITION_FINGERPRINT_OFFSET + DIGEST_WIDTH]
        .copy_from_slice(registration);
    bytes[BODY_DEFINITION_FINGERPRINT_OFFSET..BODY_DEFINITION_FINGERPRINT_OFFSET + DIGEST_WIDTH]
        .copy_from_slice(body_definition);
    bytes
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
