use super::*;
use crate::validate_darwin_arm64_object_envelope_v1;

#[test]
fn text_is_accepted_by_both_builtin_profiles() {
    for profile in [
        BuiltinLinkObjectSectionProfileV1::ScoopLir,
        BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
    ] {
        let envelope = validate_darwin_arm64_object_envelope_v1(&text_object()).unwrap();
        let inventory = validate_builtin_object_section_inventory_v1(envelope, profile).unwrap();
        assert_eq!(inventory.roles(), &[BuiltinObjectSectionRoleV1::Text]);
    }
}

#[test]
fn stackmaps_are_scoop_only() {
    let bytes = object_with_section(b"__LLVM_STACKMAPS", b"__llvm_stackmaps", macho::S_REGULAR);
    let scoop = validate_builtin_object_section_inventory_v1(
        validate_darwin_arm64_object_envelope_v1(&bytes).unwrap(),
        BuiltinLinkObjectSectionProfileV1::ScoopLir,
    )
    .unwrap();
    assert_eq!(scoop.roles(), &[BuiltinObjectSectionRoleV1::LlvmStackmaps]);

    let bridge = validate_builtin_object_section_inventory_v1(
        validate_darwin_arm64_object_envelope_v1(&bytes).unwrap(),
        BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
    );
    assert_eq!(
        bridge.unwrap_err(),
        BuiltinObjectSectionValidationError::UnsupportedSectionForProfile {
            profile: BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
            role: BuiltinObjectSectionRoleV1::LlvmStackmaps,
        }
    );
}

#[test]
fn constructors_and_flag_drift_fail_closed() {
    let constructor = object_with_section(
        b"__DATA",
        b"__mod_init_func",
        macho::S_MOD_INIT_FUNC_POINTERS,
    );
    let error = validate_builtin_object_section_inventory_v1(
        validate_darwin_arm64_object_envelope_v1(&constructor).unwrap(),
        BuiltinLinkObjectSectionProfileV1::ScoopLir,
    )
    .unwrap_err();
    assert_eq!(
        error,
        BuiltinObjectSectionValidationError::UnsupportedSectionName
    );

    let wrong_text_flags = object_with_section(b"__TEXT", b"__text", macho::S_REGULAR);
    let error = validate_builtin_object_section_inventory_v1(
        validate_darwin_arm64_object_envelope_v1(&wrong_text_flags).unwrap(),
        BuiltinLinkObjectSectionProfileV1::ScoopLir,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        BuiltinObjectSectionValidationError::SectionFlagsMismatch {
            role: BuiltinObjectSectionRoleV1::Text,
            ..
        }
    ));
}

#[test]
fn generated_bridge_requires_text_and_rejects_writable_state() {
    let read_only = object_with_section(b"__TEXT", b"__const", macho::S_REGULAR);
    let missing_text = validate_builtin_object_section_inventory_v1(
        validate_darwin_arm64_object_envelope_v1(&read_only).unwrap(),
        BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
    )
    .unwrap_err();
    assert_eq!(
        missing_text,
        BuiltinObjectSectionValidationError::MissingGeneratedBridgeText
    );

    let writable = object_with_section(b"__DATA", b"__data", macho::S_REGULAR);
    let unsupported = validate_builtin_object_section_inventory_v1(
        validate_darwin_arm64_object_envelope_v1(&writable).unwrap(),
        BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
    )
    .unwrap_err();
    assert_eq!(
        unsupported,
        BuiltinObjectSectionValidationError::UnsupportedSectionForProfile {
            profile: BuiltinLinkObjectSectionProfileV1::GeneratedCBridge,
            role: BuiltinObjectSectionRoleV1::WritableData,
        }
    );
}

fn text_object() -> Vec<u8> {
    object_with_section(
        b"__TEXT",
        b"__text",
        macho::S_REGULAR | macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS,
    )
}

fn object_with_section(segment_name: &[u8], section_name: &[u8], flags: u32) -> Vec<u8> {
    let segment_size = 152_u32;
    let symtab_size = 24_u32;
    let dysymtab_size = 80_u32;
    let deployment_size = 24_u32;
    let command_bytes = segment_size + symtab_size + dysymtab_size + deployment_size;
    let section_offset = 32 + command_bytes;
    let string_offset = section_offset + 4;
    let mut bytes = Vec::with_capacity(string_offset as usize + 1);

    push_u32(&mut bytes, macho::MH_MAGIC_64);
    push_u32(&mut bytes, macho::CPU_TYPE_ARM64);
    push_u32(&mut bytes, macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(&mut bytes, macho::MH_OBJECT);
    push_u32(&mut bytes, 4);
    push_u32(&mut bytes, command_bytes);
    push_u32(&mut bytes, macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SEGMENT_64);
    push_u32(&mut bytes, segment_size);
    bytes.extend_from_slice(&[0; 16]);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, 4);
    push_u64(&mut bytes, u64::from(section_offset));
    push_u64(&mut bytes, 4);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 0);

    push_fixed_name(&mut bytes, section_name);
    push_fixed_name(&mut bytes, segment_name);
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, 4);
    push_u32(&mut bytes, section_offset);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, flags);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, macho::LC_SYMTAB);
    push_u32(&mut bytes, symtab_size);
    push_u32(&mut bytes, section_offset + 4);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, string_offset);
    push_u32(&mut bytes, 1);

    push_u32(&mut bytes, macho::LC_DYSYMTAB);
    push_u32(&mut bytes, dysymtab_size);
    bytes.extend_from_slice(&[0; 72]);

    push_u32(&mut bytes, macho::LC_BUILD_VERSION);
    push_u32(&mut bytes, deployment_size);
    push_u32(&mut bytes, macho::PLATFORM_MACOS);
    push_u32(&mut bytes, 0x000d_0000);
    push_u32(&mut bytes, 0x000d_0000);
    push_u32(&mut bytes, 0);

    bytes.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd]);
    bytes.push(0);
    bytes
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_fixed_name(bytes: &mut Vec<u8>, name: &[u8]) {
    let mut fixed = [0; 16];
    fixed[..name.len()].copy_from_slice(name);
    bytes.extend_from_slice(&fixed);
}
