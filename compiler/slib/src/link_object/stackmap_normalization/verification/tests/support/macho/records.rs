use super::*;

pub(super) fn push_registration(
    bytes: &mut Vec<u8>,
    registration: StrongSafepointRegistrationPlanV1,
) {
    push_u64(bytes, 0x5343_4f4f_5053_5054);
    push_u32(bytes, 7);
    push_u32(bytes, u32::try_from(SAFEPOINT_REGISTRATION_SIZE).unwrap());
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(registration.site().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, registration.safepoint().get());
    push_u32(bytes, registration.role().tag());
    push_u32(bytes, registration.root_pair_count());
    bytes.extend_from_slice(registration.owner().as_array());
    bytes.extend_from_slice(&[0; 32]);
}

pub(super) fn push_callable_registration(
    bytes: &mut Vec<u8>,
    registration: StrongCallableRegistrationPlanV1,
) {
    push_u64(bytes, 0x5343_4f4f_5043_414c);
    push_u32(bytes, 7);
    push_u32(bytes, u32::try_from(CALLABLE_REGISTRATION_SIZE).unwrap());
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(registration.body().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, registration.context_key_count());
}

pub(super) fn push_type_registration(
    bytes: &mut Vec<u8>,
    registration: &StrongTypeRegistrationPlanV1,
) {
    push_u64(bytes, 0x5343_4f4f_5054_5950);
    push_u32(bytes, 7);
    push_u32(bytes, u32::try_from(TYPE_REGISTRATION_SIZE).unwrap());
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(registration.exact_type().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, registration.runtime_type().get());
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
}

pub(super) fn push_type_descriptor(
    bytes: &mut Vec<u8>,
    registration: &StrongTypeRegistrationPlanV1,
) {
    let shape = registration.semantic().instance_shape();
    push_u64(bytes, registration.runtime_type().get());
    push_u32(bytes, shape.instance_kind().tag());
    push_u32(bytes, shape.inline_storage_kind().tag());
    push_u64(bytes, shape.minimum_size());
    push_u64(bytes, shape.instance_alignment());
    push_u64(bytes, shape.inline_offset());
    push_u64(bytes, shape.inline_size());
    push_u64(bytes, shape.inline_stride());
    push_u64(bytes, shape.inline_alignment());
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(
        bytes,
        u64::try_from(registration.semantic().itables().len()).unwrap(),
    );
    push_u64(bytes, 0);
    push_u64(
        bytes,
        u64::try_from(registration.semantic().diagnostic_name().len()).unwrap(),
    );
    assert_eq!(registration.semantic().relations().runtime_kind(), 0);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, 0);
}

pub(super) fn push_immortal_registration(
    bytes: &mut Vec<u8>,
    registration: StrongImmortalObjectRegistrationPlanV1,
) {
    push_u64(bytes, 0x5343_4f4f_5049_4d4d);
    push_u32(bytes, 7);
    push_u32(bytes, u32::try_from(IMMORTAL_REGISTRATION_SIZE).unwrap());
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(registration.object().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, 0);
    push_u64(bytes, registration.object_size());
    push_u64(bytes, registration.required_alignment());
    push_u64(bytes, 0);
}

pub(super) fn push_static_storage_registration(
    bytes: &mut Vec<u8>,
    registration: &StrongStaticStorageRegistrationPlanV1,
) {
    let semantic = registration.semantic();
    push_u64(bytes, 0x5343_4f4f_5053_544f);
    push_u32(bytes, 7);
    push_u32(
        bytes,
        u32::try_from(STATIC_STORAGE_REGISTRATION_SIZE).unwrap(),
    );
    push_u32(bytes, 1);
    push_u32(bytes, 0);
    bytes.extend_from_slice(semantic.storage().as_array());
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u32(bytes, semantic.scan_kind().tag());
    push_u32(bytes, semantic.initial_state().tag());
    push_u64(bytes, 0);
    push_u64(bytes, semantic.byte_size());
    push_u64(bytes, semantic.allocation_extent());
    push_u64(bytes, semantic.required_alignment());
    push_u64(bytes, 0);
    bytes.extend_from_slice(&[0; 32]);
    bytes.extend_from_slice(&[0; 32]);
    push_u64(bytes, 0);
    push_u64(
        bytes,
        u64::try_from(semantic.initial_state().initial_template().len()).unwrap(),
    );
    push_u64(bytes, 0);
    push_u64(
        bytes,
        u64::try_from(semantic.initial_state().immortal_relocations().len()).unwrap(),
    );
}

pub(super) fn push_scan_program(
    bytes: &mut Vec<u8>,
    registration: &StrongStaticStorageRegistrationPlanV1,
) {
    match registration.semantic().scan_program() {
        scoop_lir::RefScan::None => push_u64(bytes, 0),
        scoop_lir::RefScan::References(offsets) => {
            push_u64(bytes, u64::try_from(offsets.len()).unwrap());
            for offset in offsets {
                push_u64(bytes, *offset);
            }
        }
        scoop_lir::RefScan::Sequence(_) | scoop_lir::RefScan::Array { .. } => {
            panic!("fixture cannot contain non-value scans")
        }
    }
}

pub(super) fn push_immortal_object(
    bytes: &mut Vec<u8>,
    module: &scoop_lir::Module,
    registration: StrongImmortalObjectRegistrationPlanV1,
) {
    let (identity, value) = module
        .globals
        .iter()
        .find_map(|(_, global)| match &global.init {
            scoop_lir::GlobalInit::StringConst { identity, value }
                if identity.identity_record().id() == registration.object() =>
            {
                Some((identity, value))
            }
            scoop_lir::GlobalInit::StringConst { .. }
            | scoop_lir::GlobalInit::Storage { .. }
            | scoop_lir::GlobalInit::CString { .. }
            | scoop_lir::GlobalInit::RawStorage { .. }
            | scoop_lir::GlobalInit::ImportedStorage { .. } => None,
        })
        .expect("immortal registration must resolve to a StringConst global");
    assert_eq!(identity.identity_record().id(), registration.object());
    let start = bytes.len();
    push_u64(bytes, 0);
    push_u64(bytes, 0);
    push_u64(bytes, u64::try_from(value.len()).unwrap());
    bytes.extend_from_slice(value.as_bytes());
    bytes.resize(
        start + usize::try_from(registration.object_size()).unwrap(),
        0,
    );
}
