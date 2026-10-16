use super::*;
use crate::link_object::DarwinBuildToolVersionV1;

#[test]
fn reads_scoop_object_sections() {
    let qualified = validate_scoop_lir_llvm_22_1_object_envelope_v1(
        scoop_target_data::DARWIN_AARCH64,
        &object(None),
    )
    .unwrap();
    assert_eq!(
        qualified.sections().roles(),
        &[crate::link_object::BuiltinObjectSectionRoleV1::Text]
    );
}

fn object(deployment: Option<(u32, u32, &[DarwinBuildToolVersionV1])>) -> Vec<u8> {
    let segment_size = 152_u32;
    let symtab_size = 24_u32;
    let dysymtab_size = 80_u32;
    let deployment_size = deployment
        .map(|(_, _, tools)| 24 + u32::try_from(tools.len()).unwrap() * 8)
        .unwrap_or(0);
    let command_count = 3 + u32::from(deployment.is_some());
    let command_bytes = segment_size + symtab_size + dysymtab_size + deployment_size;
    let section_offset = 32 + command_bytes;
    let string_offset = section_offset + 4;
    let mut bytes = Vec::with_capacity(string_offset as usize + 1);

    push_u32(&mut bytes, object::macho::MH_MAGIC_64);
    push_u32(&mut bytes, object::macho::CPU_TYPE_ARM64);
    push_u32(&mut bytes, object::macho::CPU_SUBTYPE_ARM64_ALL);
    push_u32(&mut bytes, object::macho::MH_OBJECT);
    push_u32(&mut bytes, command_count);
    push_u32(&mut bytes, command_bytes);
    push_u32(&mut bytes, object::macho::MH_SUBSECTIONS_VIA_SYMBOLS);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, object::macho::LC_SEGMENT_64);
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

    push_fixed_name(&mut bytes, b"__text");
    push_fixed_name(&mut bytes, b"__TEXT");
    push_u64(&mut bytes, 0);
    push_u64(&mut bytes, 4);
    push_u32(&mut bytes, section_offset);
    push_u32(&mut bytes, 2);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(
        &mut bytes,
        object::macho::S_REGULAR
            | object::macho::S_ATTR_PURE_INSTRUCTIONS
            | object::macho::S_ATTR_SOME_INSTRUCTIONS,
    );
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    push_u32(&mut bytes, object::macho::LC_SYMTAB);
    push_u32(&mut bytes, symtab_size);
    push_u32(&mut bytes, section_offset + 4);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, string_offset);
    push_u32(&mut bytes, 1);

    push_u32(&mut bytes, object::macho::LC_DYSYMTAB);
    push_u32(&mut bytes, dysymtab_size);
    bytes.extend_from_slice(&[0; 72]);

    if let Some((minimum_os, sdk, tools)) = deployment {
        push_u32(&mut bytes, object::macho::LC_BUILD_VERSION);
        push_u32(&mut bytes, deployment_size);
        push_u32(&mut bytes, object::macho::PLATFORM_MACOS);
        push_u32(&mut bytes, minimum_os);
        push_u32(&mut bytes, sdk);
        push_u32(&mut bytes, u32::try_from(tools.len()).unwrap());
        for tool in tools {
            push_u32(&mut bytes, tool.tool());
            push_u32(&mut bytes, tool.version());
        }
    }

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

#[test]
fn canonical_unknown_target_is_rejected_before_object_parsing() {
    const UNKNOWN: scoop_lir::TargetProfileId =
        match scoop_lir::TargetProfileId::new("unknown-target", 1) {
            Ok(id) => id,
            Err(_) => panic!("canonical test identity"),
        };
    let target =
        scoop_lir::LirTargetProfile::new(&UNKNOWN, scoop_target_data::DARWIN_AARCH64.description());
    assert!(matches!(
        validate_scoop_lir_llvm_22_1_object_envelope_v1(target, &[]),
        Err(ScoopLirObjectEnvelopeValidationError::UnsupportedTarget)
    ));
    assert!(crate::ElfArchitecture::for_target(UNKNOWN).is_err());
}
