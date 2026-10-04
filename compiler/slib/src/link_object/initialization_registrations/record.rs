use scoop_lir::StrongInitializationUnitRegistrationPlan;

use super::{InitializationArtifactRoleV1, StrongInitializationRegistrationValidationError};

const DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5049_4e49;
pub(in crate::link_object) const ABI_VERSION: u32 = 4;
const DEFINITION_FINGERPRINT_OFFSET: usize = 120;
const GATEWAY_DEFINITION_FINGERPRINT_OFFSET: usize = 312;
const DIGEST_WIDTH: usize = 32;
pub(super) const CELL_SIZE: usize = 16;
pub(in crate::link_object) const DESCRIPTOR_SIZE: usize = 352;

pub(super) fn validate_cell_bytes<D>(
    object: &[u8],
    file_start: u64,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<(), StrongInitializationRegistrationValidationError> {
    validate_bytes(
        object,
        file_start,
        &[0; CELL_SIZE],
        plan,
        InitializationArtifactRoleV1::Cell,
    )
}

pub(super) fn validate_record_bytes<D>(
    object: &[u8],
    file_start: u64,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<(), StrongInitializationRegistrationValidationError> {
    validate_bytes(
        object,
        file_start,
        &expected_record(plan),
        plan,
        InitializationArtifactRoleV1::Registration,
    )
}

fn validate_bytes<D>(
    object: &[u8],
    file_start: u64,
    expected: &[u8],
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    role: InitializationArtifactRoleV1,
) -> Result<(), StrongInitializationRegistrationValidationError> {
    let unit = plan.semantic().unit();
    let start = usize::try_from(file_start)
        .map_err(|_| StrongInitializationRegistrationValidationError::RecordRangeOverflow(unit))?;
    let end = start
        .checked_add(expected.len())
        .ok_or(StrongInitializationRegistrationValidationError::RecordRangeOverflow(unit))?;
    let actual = object
        .get(start..end)
        .ok_or(StrongInitializationRegistrationValidationError::RecordRangeOverflow(unit))?;
    if let Some(offset) = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(
            StrongInitializationRegistrationValidationError::RecordByteMismatch {
                unit,
                role,
                offset_within_atom: u16::try_from(offset)
                    .expect("initialization artifact offset fits u16"),
                expected: expected[offset],
                actual: actual[offset],
            },
        );
    }
    Ok(())
}

pub(super) fn expected_record<D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> [u8; DESCRIPTOR_SIZE] {
    let semantic = plan.semantic();
    let mut bytes = [0; DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, ABI_VERSION);
    write_u32(&mut bytes, 12, DESCRIPTOR_SIZE as u32);
    bytes[16..152].copy_from_slice(
        &super::super::registration_identity::provisional_registration_identity(
            semantic.unit().as_array(),
            plan.definition_owner(),
        ),
    );
    write_u32(&mut bytes, 152, semantic.schedule().tag());
    write_u64(
        &mut bytes,
        168,
        u64::try_from(semantic.diagnostic_path().len()).unwrap(),
    );
    bytes[200..232].copy_from_slice(semantic.initializer().as_array());
    bytes[232..264].copy_from_slice(semantic.ensure().as_array());
    if let Some(gateway) = semantic.schedule().gateway() {
        bytes[280..312].copy_from_slice(gateway.as_array());
    }
    bytes
}

pub(in crate::link_object) fn expected_final_record<D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
    registration: &[u8; DIGEST_WIDTH],
    gateway_definition: Option<&[u8; DIGEST_WIDTH]>,
) -> [u8; DESCRIPTOR_SIZE] {
    let mut bytes = expected_record(plan);
    bytes[DEFINITION_FINGERPRINT_OFFSET..DEFINITION_FINGERPRINT_OFFSET + DIGEST_WIDTH]
        .copy_from_slice(registration);
    if let Some(gateway) = gateway_definition {
        bytes[GATEWAY_DEFINITION_FINGERPRINT_OFFSET
            ..GATEWAY_DEFINITION_FINGERPRINT_OFFSET + DIGEST_WIDTH]
            .copy_from_slice(gateway);
    }
    bytes
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
