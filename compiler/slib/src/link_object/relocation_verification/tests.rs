use super::super::symbol_verification::tests::{fixture, object_for_plan_with_branch_relocation};
use super::*;
use crate::{
    PlannedStrongObjectSymbolRoleV1, validate_scoop_lir_llvm_22_1_object_envelope_v1,
    verify_member_strong_object_definitions_v1,
};

#[test]
fn assigns_relocations_to_exact_atoms_and_resolves_primary_targets() {
    let fixture = fixture();
    let primary = primary_role(&fixture.symbols);
    let object = object_for_plan_with_branch_relocation(
        &fixture.symbols,
        canonical_value,
        Some((0, primary)),
    );
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&object.bytes)
        .unwrap()
        .into_sections();
    let definitions =
        verify_member_strong_object_definitions_v1(&object.bytes, sections, &fixture.symbols)
            .unwrap();

    let verified = verify_member_object_relocations_v1(definitions).unwrap();

    assert_eq!(verified.member(), fixture.symbols.member());
    assert_eq!(verified.relocations().len(), 1);
    let relocation = &verified.relocations()[0];
    assert_eq!(relocation.containing_atom(), fixture.atom);
    assert_eq!(
        relocation.containing_atom_role(),
        scoop_identity::DefinitionAtomRole::Primary
    );
    assert_eq!(relocation.section_role(), BuiltinObjectSectionRoleV1::Text);
    assert_eq!(relocation.offset_within_atom(), 0);
    assert_eq!(relocation.width_bytes(), 4);
    assert_eq!(relocation.encoded_value(), 0xddcc_bbaa);
    assert_eq!(
        relocation.shape().form(),
        VerifiedDarwinArm64RelocationFormV1::Branch26
    );
    assert_eq!(
        relocation.shape(),
        &VerifiedDarwinArm64RelocationShapeV1::Branch26 {
            target: VerifiedRelocationTargetV1::StrongDefinition {
                definition: fixture.plan,
            },
        }
    );
}

#[test]
fn rejects_boundary_targets_and_relocations_in_padding() {
    let fixture = fixture();
    let start = fixture
        .symbols
        .symbols()
        .iter()
        .find_map(|symbol| match symbol.role() {
            role @ PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => Some(role),
            _ => None,
        })
        .unwrap();
    let object =
        object_for_plan_with_branch_relocation(&fixture.symbols, canonical_value, Some((0, start)));
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&object.bytes)
        .unwrap()
        .into_sections();
    let definitions =
        verify_member_strong_object_definitions_v1(&object.bytes, sections, &fixture.symbols)
            .unwrap();
    assert_eq!(
        verify_member_object_relocations_v1(definitions),
        Err(ObjectRelocationValidationError::BoundaryRelocationTarget {
            atom: fixture.atom,
            boundary: VerifiedBoundaryRoleV1::Start,
        })
    );

    let object = object_for_plan_with_branch_relocation(
        &fixture.symbols,
        canonical_value,
        Some((4, primary_role(&fixture.symbols))),
    );
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&object.bytes)
        .unwrap()
        .into_sections();
    let definitions =
        verify_member_strong_object_definitions_v1(&object.bytes, sections, &fixture.symbols)
            .unwrap();
    assert_eq!(
        verify_member_object_relocations_v1(definitions),
        Err(ObjectRelocationValidationError::OrphanRelocation {
            section: std::num::NonZeroU32::new(1).unwrap(),
            offset: 4,
            width_bytes: 4,
        })
    );
}

#[test]
fn classifies_used_undefined_symbols_and_rejects_unused_entries() {
    let fixture = fixture();
    let object = object_for_plan_with_branch_relocation(
        &fixture.symbols,
        canonical_value,
        Some((0, primary_role(&fixture.symbols))),
    );
    let used_bytes = add_undefined_symbol(object.bytes.clone(), true, b"_external");
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&used_bytes)
        .unwrap()
        .into_sections();
    let definitions =
        verify_member_strong_object_definitions_v1(&used_bytes, sections, &fixture.symbols)
            .unwrap();
    let verified = verify_member_object_relocations_v1(definitions).unwrap();
    assert_eq!(
        verified.relocations()[0].shape(),
        &VerifiedDarwinArm64RelocationShapeV1::Branch26 {
            target: VerifiedRelocationTargetV1::ExternalUndefined {
                table_index: 3,
                name: b"_external".to_vec(),
            },
        }
    );

    let unused_bytes = add_undefined_symbol(object.bytes, false, b"_external");
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&unused_bytes)
        .unwrap()
        .into_sections();
    let definitions =
        verify_member_strong_object_definitions_v1(&unused_bytes, sections, &fixture.symbols)
            .unwrap();
    assert_eq!(
        verify_member_object_relocations_v1(definitions),
        Err(ObjectRelocationValidationError::UnusedExternalUndefined {
            table_index: 3,
            name: b"_external".to_vec(),
        })
    );
}

#[test]
fn assigns_local_machine_symbols_to_atom_owners() {
    let fixture = fixture();
    let object = object_for_plan_with_branch_relocation(
        &fixture.symbols,
        canonical_value,
        Some((0, primary_role(&fixture.symbols))),
    );
    let local_bytes = add_local_symbol(object.bytes.clone(), 2);
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&local_bytes)
        .unwrap()
        .into_sections();
    let definitions =
        verify_member_strong_object_definitions_v1(&local_bytes, sections, &fixture.symbols)
            .unwrap();
    let verified = verify_member_object_relocations_v1(definitions).unwrap();
    assert_eq!(
        verified.relocations()[0].shape(),
        &VerifiedDarwinArm64RelocationShapeV1::Branch26 {
            target: VerifiedRelocationTargetV1::LocalDefinition {
                table_index: 0,
                name: b"ltmp0".to_vec(),
                owner_atom: fixture.atom,
                section_ordinal: std::num::NonZeroU8::new(1).unwrap(),
                value: 2,
            },
        }
    );

    let unowned_bytes = add_local_symbol(object.bytes, 6);
    let sections = validate_scoop_lir_llvm_22_1_object_envelope_v1(&unowned_bytes)
        .unwrap()
        .into_sections();
    let definitions =
        verify_member_strong_object_definitions_v1(&unowned_bytes, sections, &fixture.symbols)
            .unwrap();
    assert_eq!(
        verify_member_object_relocations_v1(definitions),
        Err(ObjectRelocationValidationError::UnownedLocalDefinition { table_index: 0 })
    );
}

fn primary_role(
    symbols: &crate::PlannedMemberStrongObjectSymbolsV1,
) -> PlannedStrongObjectSymbolRoleV1 {
    symbols
        .symbols()
        .iter()
        .find_map(|symbol| match symbol.role() {
            role @ PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. } => Some(role),
            _ => None,
        })
        .unwrap()
}

fn canonical_value(role: PlannedStrongObjectSymbolRoleV1) -> u64 {
    match role {
        PlannedStrongObjectSymbolRoleV1::PrimaryDefinition { .. }
        | PlannedStrongObjectSymbolRoleV1::AtomBoundaryStart { .. } => 0,
        PlannedStrongObjectSymbolRoleV1::AtomBoundaryEnd { .. } => 4,
    }
}

pub(in crate::link_object) fn add_undefined_symbol(
    mut bytes: Vec<u8>,
    retarget_relocation: bool,
    undefined_name: &[u8],
) -> Vec<u8> {
    const SYMTAB_COMMAND: usize = 32 + 152;
    const DYSYMTAB_COMMAND: usize = SYMTAB_COMMAND + 24;
    const SECTION_RECORD: usize = 32 + 72;

    let symbol_count = read_u32(&bytes, SYMTAB_COMMAND + 12);
    let string_offset = read_u32(&bytes, SYMTAB_COMMAND + 16) as usize;
    let string_size = read_u32(&bytes, SYMTAB_COMMAND + 20);
    bytes.splice(string_offset..string_offset, [0; 16]);
    write_u32(&mut bytes, SYMTAB_COMMAND + 12, symbol_count + 1);
    write_u32(
        &mut bytes,
        SYMTAB_COMMAND + 16,
        u32::try_from(string_offset + 16).unwrap(),
    );
    write_u32(
        &mut bytes,
        SYMTAB_COMMAND + 20,
        string_size + undefined_name.len() as u32 + 1,
    );
    write_u32(&mut bytes, DYSYMTAB_COMMAND + 24, symbol_count);
    write_u32(&mut bytes, DYSYMTAB_COMMAND + 28, 1);

    write_u32(&mut bytes, string_offset, string_size);
    bytes[string_offset + 4] = object::macho::N_UNDF | object::macho::N_EXT;
    bytes.extend_from_slice(undefined_name);
    bytes.push(0);

    if retarget_relocation {
        let relocation_offset = read_u32(&bytes, SECTION_RECORD + 56) as usize;
        let fields = read_u32(&bytes, relocation_offset + 4);
        write_u32(
            &mut bytes,
            relocation_offset + 4,
            (fields & 0xff00_0000) | symbol_count,
        );
    }
    bytes
}

fn add_local_symbol(mut bytes: Vec<u8>, value: u64) -> Vec<u8> {
    const SYMTAB_COMMAND: usize = 32 + 152;
    const DYSYMTAB_COMMAND: usize = SYMTAB_COMMAND + 24;
    const SECTION_RECORD: usize = 32 + 72;
    const LOCAL_NAME: &[u8] = b"ltmp0\0";

    let symbol_offset = read_u32(&bytes, SYMTAB_COMMAND + 8) as usize;
    let symbol_count = read_u32(&bytes, SYMTAB_COMMAND + 12);
    let string_offset = read_u32(&bytes, SYMTAB_COMMAND + 16) as usize;
    let string_size = read_u32(&bytes, SYMTAB_COMMAND + 20);
    bytes.splice(symbol_offset..symbol_offset, [0; 16]);
    write_u32(&mut bytes, SYMTAB_COMMAND + 12, symbol_count + 1);
    write_u32(
        &mut bytes,
        SYMTAB_COMMAND + 16,
        u32::try_from(string_offset + 16).unwrap(),
    );
    write_u32(
        &mut bytes,
        SYMTAB_COMMAND + 20,
        string_size + LOCAL_NAME.len() as u32,
    );
    write_u32(&mut bytes, DYSYMTAB_COMMAND + 12, 1);
    write_u32(&mut bytes, DYSYMTAB_COMMAND + 16, 1);
    write_u32(&mut bytes, DYSYMTAB_COMMAND + 20, symbol_count);
    write_u32(&mut bytes, DYSYMTAB_COMMAND + 24, symbol_count + 1);

    write_u32(&mut bytes, symbol_offset, string_size);
    bytes[symbol_offset + 4] = object::macho::N_SECT;
    bytes[symbol_offset + 5] = 1;
    write_u64(&mut bytes, symbol_offset + 8, value);
    bytes.extend_from_slice(LOCAL_NAME);

    let relocation_offset = read_u32(&bytes, SECTION_RECORD + 56) as usize;
    let fields = read_u32(&bytes, relocation_offset + 4);
    write_u32(&mut bytes, relocation_offset + 4, fields & 0xff00_0000);
    bytes
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
