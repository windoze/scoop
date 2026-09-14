use scoop_lir::StrongImmortalObjectRegistrationPlanV1;

use super::StrongImmortalObjectRegistrationValidationError;

const DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5049_4d4d;
const ABI_VERSION: u32 = 1;
const STRONG_LINKAGE: u32 = 1;
const DEFINITION_FINGERPRINT_OFFSET: usize = 120;
const DIGEST_WIDTH: usize = 32;
pub(super) const DESCRIPTOR_SIZE: usize = 184;

pub(super) fn validate_record_bytes(
    object: &[u8],
    file_start: u64,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> Result<(), StrongImmortalObjectRegistrationValidationError> {
    let start = usize::try_from(file_start).map_err(|_| {
        StrongImmortalObjectRegistrationValidationError::RecordRangeOverflow(plan.object())
    })?;
    let end = start.checked_add(DESCRIPTOR_SIZE).ok_or(
        StrongImmortalObjectRegistrationValidationError::RecordRangeOverflow(plan.object()),
    )?;
    let actual = object.get(start..end).ok_or(
        StrongImmortalObjectRegistrationValidationError::RecordRangeOverflow(plan.object()),
    )?;
    let expected = expected_record(plan);
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| *actual != expected)
    {
        return Err(
            StrongImmortalObjectRegistrationValidationError::RecordByteMismatch {
                object: plan.object(),
                offset_within_atom: u16::try_from(offset)
                    .expect("immortal descriptor offset fits u16"),
                expected: expected[offset],
                actual: actual[offset],
            },
        );
    }
    Ok(())
}

pub(super) fn expected_record(
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = [0; DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, ABI_VERSION);
    write_u32(&mut bytes, 12, DESCRIPTOR_SIZE as u32);
    write_u32(&mut bytes, 16, STRONG_LINKAGE);
    bytes[24..56].copy_from_slice(plan.object().as_array());
    write_u64(&mut bytes, 160, plan.object_size());
    write_u64(&mut bytes, 168, plan.required_alignment());
    bytes
}

pub(in crate::link_object) fn expected_final_record(
    plan: StrongImmortalObjectRegistrationPlanV1,
    registration: &[u8; DIGEST_WIDTH],
) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = expected_record(plan);
    bytes[DEFINITION_FINGERPRINT_OFFSET..DEFINITION_FINGERPRINT_OFFSET + DIGEST_WIDTH]
        .copy_from_slice(registration);
    bytes
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
