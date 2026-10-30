use super::*;
use crate::validate_macho_object_envelope_v1;

#[test]
fn classifies_sections_by_actual_attributes() {
    use BuiltinObjectSectionRoleV1 as Role;
    for (segment, name, flags, expected) in [
        (
            b"__TEXT".as_slice(),
            b"__text".as_slice(),
            macho::S_ATTR_PURE_INSTRUCTIONS,
            Role::Text,
        ),
        (
            b"__TEXT",
            b"__scoop_sig",
            macho::S_REGULAR,
            Role::ReadOnlyData,
        ),
        (
            b"__DATA",
            b"__mod_init_func",
            macho::S_MOD_INIT_FUNC_POINTERS,
            Role::WritableData,
        ),
        (b"__DATA", b"__other", macho::S_REGULAR, Role::WritableData),
        (
            b"__TEXT",
            b"__cstring",
            macho::S_CSTRING_LITERALS,
            Role::CString,
        ),
        (
            b"__LLVM_STACKMAPS",
            b"__llvm_stackmaps",
            macho::S_REGULAR,
            Role::LlvmStackmaps,
        ),
        (
            b"__TEXT",
            b"__literal8",
            macho::S_8BYTE_LITERALS,
            Role::ReadOnlyData,
        ),
    ] {
        let bytes = object_with_section(segment, name, flags);
        let inventory = read_builtin_object_sections_v1(
            validate_macho_object_envelope_v1(scoop_target_data::DARWIN_AARCH64_ID, &bytes)
                .unwrap(),
        );
        assert_eq!(inventory.roles(), &[expected]);
    }
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
