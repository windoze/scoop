use super::*;
use scoop_lir::{
    DarwinBuildToolIdV1, DarwinBuildToolVersionContractV1, DarwinCBridgeDeploymentContractV1,
    DarwinPackedVersionV1,
};

#[test]
fn llvm_22_1_scoop_profile_requires_no_deployment_command() {
    let qualified = validate_scoop_lir_llvm_22_1_object_envelope_v1(&object(None)).unwrap();
    assert_eq!(
        qualified.sections().roles(),
        &[super::super::BuiltinObjectSectionRoleV1::Text]
    );

    let error = validate_scoop_lir_llvm_22_1_object_envelope_v1(&object(Some((
        0x000d_0000,
        0x000d_0000,
        &[],
    ))))
    .unwrap_err();
    assert_eq!(
        error,
        ScoopLirObjectEnvelopeValidationError::UnexpectedDeployment(
            DarwinDeploymentCommandV1::BuildVersion {
                minimum_os: 0x000d_0000,
                sdk: 0x000d_0000,
                tools: Vec::new(),
            }
        )
    );
}

#[test]
fn generated_c_profile_requires_the_exact_deployment_contract() {
    let tools = [DarwinBuildToolVersionV1::new(
        object::macho::TOOL_CLANG,
        0x1000_0200,
    )];
    let contract = deployment_contract(0x000d_0100, 0x000e_0200, 0x1000_0200);
    let bytes = object(Some((0x000d_0100, 0x000e_0200, &tools)));
    let qualified = validate_generated_c_bridge_object_envelope_v1(&bytes, &contract).unwrap();
    assert_eq!(
        qualified.sections().roles(),
        &[super::super::BuiltinObjectSectionRoleV1::Text]
    );

    assert_eq!(
        validate_generated_c_bridge_object_envelope_v1(&object(None), &contract),
        Err(GeneratedCBridgeObjectEnvelopeValidationError::MissingDeployment)
    );
    let wrong_sdk = deployment_contract(0x000d_0100, 0x000e_0300, 0x1000_0200);
    assert!(matches!(
        validate_generated_c_bridge_object_envelope_v1(&bytes, &wrong_sdk),
        Err(GeneratedCBridgeObjectEnvelopeValidationError::DeploymentMismatch { .. })
    ));
}

#[test]
fn generated_c_deployment_contract_rejects_noncanonical_tools() {
    let version = DarwinPackedVersionV1::new(1).unwrap();
    let clang = DarwinBuildToolVersionContractV1::new(DarwinBuildToolIdV1::Clang, version);
    let linker = DarwinBuildToolVersionContractV1::new(DarwinBuildToolIdV1::Ld, version);
    assert_eq!(
        DarwinCBridgeDeploymentContractV1::new(version, version, vec![linker, clang]),
        Err(scoop_lir::DarwinCBridgeDeploymentContractError::NonCanonicalToolOrder { index: 1 })
    );
    assert_eq!(
        DarwinCBridgeDeploymentContractV1::new(version, version, vec![clang, clang]),
        Err(
            scoop_lir::DarwinCBridgeDeploymentContractError::DuplicateTool(
                DarwinBuildToolIdV1::Clang,
            )
        )
    );
}

fn deployment_contract(
    minimum_os: u32,
    sdk: u32,
    clang_version: u32,
) -> DarwinCBridgeDeploymentContractV1 {
    DarwinCBridgeDeploymentContractV1::new(
        DarwinPackedVersionV1::new(minimum_os).unwrap(),
        DarwinPackedVersionV1::new(sdk).unwrap(),
        vec![DarwinBuildToolVersionContractV1::new(
            DarwinBuildToolIdV1::Clang,
            DarwinPackedVersionV1::new(clang_version).unwrap(),
        )],
    )
    .unwrap()
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
