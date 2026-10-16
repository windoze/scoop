use scoop_lir::RegistrationDefinitionOwner;
use scoop_lir::StrongInitializationUnitRegistrationPlan;
const DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5049_4e49;
const ABI_VERSION: u32 = scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1;
const DESCRIPTOR_SIZE: usize = 320;

pub(super) fn initialization_record<D>(
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> [u8; DESCRIPTOR_SIZE] {
    let semantic = plan.semantic();
    let mut bytes = [0; DESCRIPTOR_SIZE];
    write_u64(&mut bytes, 0, DESCRIPTOR_MAGIC);
    write_u32(&mut bytes, 8, ABI_VERSION);
    write_u32(&mut bytes, 12, DESCRIPTOR_SIZE as u32);
    bytes[16..120].copy_from_slice(&provisional_registration_identity(
        semantic.unit().as_array(),
        plan.definition_owner(),
    ));
    write_u32(&mut bytes, 120, semantic.schedule().tag());
    write_u64(
        &mut bytes,
        136,
        u64::try_from(semantic.diagnostic_path().len()).unwrap(),
    );
    bytes[168..200].copy_from_slice(semantic.initializer().as_array());
    bytes[200..232].copy_from_slice(semantic.ensure().as_array());
    if let Some(gateway) = semantic.schedule().gateway() {
        bytes[248..280].copy_from_slice(gateway.as_array());
    }
    bytes
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn callable_record(plan: scoop_lir::StrongCallableRegistrationPlanV1) -> [u8; 176] {
    let mut bytes = [0; 176];
    bytes[0..8].copy_from_slice(&0x5343_4f4f_5043_414c_u64.to_le_bytes());
    bytes[8..12].copy_from_slice(&scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1.to_le_bytes());
    bytes[12..16].copy_from_slice(&176_u32.to_le_bytes());
    bytes[168..176].copy_from_slice(&plan.context_key_count().to_le_bytes());
    bytes[16..120].copy_from_slice(&provisional_registration_identity(
        plan.body().as_array(),
        plan.definition_owner(),
    ));
    bytes
}

fn provisional_registration_identity(
    semantic: &[u8; 32],
    owner: RegistrationDefinitionOwner,
) -> [u8; 104] {
    let mut bytes = [0; 104];
    let linkage = match owner {
        RegistrationDefinitionOwner::Strong => 1_u32,
        RegistrationDefinitionOwner::Odr { group, member } => {
            bytes[40..72].copy_from_slice(group.as_array());
            bytes[72..104].copy_from_slice(member.as_array());
            2
        }
    };
    bytes[..4].copy_from_slice(&linkage.to_le_bytes());
    bytes[8..40].copy_from_slice(semantic);
    bytes
}
