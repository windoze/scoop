use scoop_lir::StrongSafepointRegistrationPlanV1;

use super::StrongSafepointRegistrationValidationError;

const DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5053_5054;
const ABI_VERSION: u32 = 1;
const STRONG_LINKAGE: u32 = 1;
pub(super) const DESCRIPTOR_SIZE: usize = 232;

pub(super) fn validate_record_bytes(
    object: &[u8],
    file_start: u64,
    plan: StrongSafepointRegistrationPlanV1,
) -> Result<(), StrongSafepointRegistrationValidationError> {
    let start = usize::try_from(file_start).map_err(|_| {
        StrongSafepointRegistrationValidationError::RecordRangeOverflow(plan.site())
    })?;
    let end = start
        .checked_add(DESCRIPTOR_SIZE)
        .ok_or(StrongSafepointRegistrationValidationError::RecordRangeOverflow(plan.site()))?;
    let actual = object
        .get(start..end)
        .ok_or(StrongSafepointRegistrationValidationError::RecordRangeOverflow(plan.site()))?;
    let expected = expected_record(plan);
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| *actual != expected)
    {
        return Err(
            StrongSafepointRegistrationValidationError::RecordByteMismatch {
                site: plan.site(),
                offset_within_atom: u16::try_from(offset).expect("descriptor offset fits u16"),
                expected: expected[offset],
                actual: actual[offset],
            },
        );
    }
    Ok(())
}

fn expected_record(plan: StrongSafepointRegistrationPlanV1) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = [0; DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, ABI_VERSION);
    write_u32(&mut bytes, 12, DESCRIPTOR_SIZE as u32);
    write_u32(&mut bytes, 16, STRONG_LINKAGE);
    bytes[24..56].copy_from_slice(plan.site().as_array());
    write_u64(&mut bytes, 152, plan.safepoint().get());
    write_u32(&mut bytes, 160, plan.role().tag());
    write_u32(&mut bytes, 164, plan.root_pair_count());
    bytes[168..200].copy_from_slice(plan.owner().as_array());
    bytes
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
