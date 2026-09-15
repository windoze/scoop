use super::*;
use crate::{ObjectEnvelopeValidationError, validate_darwin_arm64_object_envelope_v1};

const DYSYMTAB_OFFSET: usize = 32 + 152 + 24;
const SYMBOL_OFFSET: usize = 32 + 152 + 24 + 80 + 24 + 4 + 8;

#[test]
fn normalizes_strong_definition_and_no_dead_strip() {
    let bytes = super::super::tests::object_with_text_section();
    let envelope = validate_darwin_arm64_object_envelope_v1(&bytes).unwrap();
    assert_eq!(envelope.symbols().len(), 1);
    let symbol = &envelope.symbols()[0];
    assert_eq!(symbol.table_index(), 0);
    assert_eq!(symbol.name(), b"_f");
    assert_eq!(
        symbol.kind(),
        DarwinArm64SymbolKindV1::ExternalStrongDefinition
    );
    assert_eq!(symbol.section_ordinal(), NonZeroU8::new(1));
    assert_eq!(symbol.value(), 0);
    assert!(!symbol.no_dead_strip());
    assert!(!symbol.private_external());

    let mut retained = bytes;
    write_u16(&mut retained, SYMBOL_OFFSET + 6, macho::N_NO_DEAD_STRIP);
    let retained = validate_darwin_arm64_object_envelope_v1(&retained).unwrap();
    assert!(retained.symbols()[0].no_dead_strip());

    let mut alternate = super::super::tests::object_with_text_section();
    write_u16(&mut alternate, SYMBOL_OFFSET + 6, macho::N_ALT_ENTRY);
    let alternate = validate_darwin_arm64_object_envelope_v1(&alternate).unwrap();
    assert!(!alternate.symbols()[0].no_dead_strip());

    let mut retained_alternate = super::super::tests::object_with_text_section();
    write_u16(
        &mut retained_alternate,
        SYMBOL_OFFSET + 6,
        macho::N_NO_DEAD_STRIP | macho::N_ALT_ENTRY,
    );
    let retained_alternate = validate_darwin_arm64_object_envelope_v1(&retained_alternate).unwrap();
    assert!(retained_alternate.symbols()[0].no_dead_strip());

    let mut private_external = super::super::tests::object_with_text_section();
    private_external[SYMBOL_OFFSET + 4] |= macho::N_PEXT;
    let private_external = validate_darwin_arm64_object_envelope_v1(&private_external).unwrap();
    assert_eq!(
        private_external.symbols()[0].kind(),
        DarwinArm64SymbolKindV1::ExternalStrongDefinition
    );
    assert!(private_external.symbols()[0].private_external());
}

#[test]
fn accepts_only_nonlazy_external_undefined_symbols() {
    let mut bytes = super::super::tests::object_with_text_section();
    bytes[SYMBOL_OFFSET + 4] = macho::N_UNDF | macho::N_EXT;
    bytes[SYMBOL_OFFSET + 5] = macho::NO_SECT;
    write_u32(&mut bytes, DYSYMTAB_OFFSET + 20, 0);
    write_u32(&mut bytes, DYSYMTAB_OFFSET + 24, 0);
    write_u32(&mut bytes, DYSYMTAB_OFFSET + 28, 1);

    let envelope = validate_darwin_arm64_object_envelope_v1(&bytes).unwrap();
    assert_eq!(
        envelope.symbols()[0].kind(),
        DarwinArm64SymbolKindV1::ExternalUndefined
    );
    assert_eq!(envelope.symbols()[0].section_ordinal(), None);

    write_u64(&mut bytes, SYMBOL_OFFSET + 8, 4);
    assert_symbol_error(
        &bytes,
        DarwinArm64SymbolInventoryValidationError::InvalidUndefinedSymbol { index: 0 },
    );
}

#[test]
fn rejects_empty_names_weakness_and_unsupported_symbol_forms() {
    let mut empty_name = super::super::tests::object_with_text_section();
    write_u32(&mut empty_name, SYMBOL_OFFSET, 0);
    assert_symbol_error(
        &empty_name,
        DarwinArm64SymbolInventoryValidationError::InvalidSymbolName { index: 0 },
    );

    let mut weak = super::super::tests::object_with_text_section();
    write_u16(&mut weak, SYMBOL_OFFSET + 6, macho::N_WEAK_DEF);
    assert_symbol_error(
        &weak,
        DarwinArm64SymbolInventoryValidationError::UnsupportedSymbolDescription {
            index: 0,
            actual: macho::N_WEAK_DEF,
        },
    );

    let mut absolute = super::super::tests::object_with_text_section();
    absolute[SYMBOL_OFFSET + 4] = macho::N_ABS | macho::N_EXT;
    assert_symbol_error(
        &absolute,
        DarwinArm64SymbolInventoryValidationError::UnsupportedSymbolType {
            index: 0,
            actual: macho::N_ABS | macho::N_EXT,
        },
    );
}

#[test]
fn rejects_invalid_definition_locations_and_partition_kinds() {
    let mut bad_section = super::super::tests::object_with_text_section();
    bad_section[SYMBOL_OFFSET + 5] = 2;
    assert_symbol_error(
        &bad_section,
        DarwinArm64SymbolInventoryValidationError::InvalidDefinitionSection {
            index: 0,
            section_ordinal: 2,
        },
    );

    let mut bad_value = super::super::tests::object_with_text_section();
    write_u64(&mut bad_value, SYMBOL_OFFSET + 8, 5);
    assert_symbol_error(
        &bad_value,
        DarwinArm64SymbolInventoryValidationError::DefinitionValueOutOfBounds { index: 0 },
    );

    let mut local_in_external_partition = super::super::tests::object_with_text_section();
    local_in_external_partition[SYMBOL_OFFSET + 4] = macho::N_SECT;
    assert_symbol_error(
        &local_in_external_partition,
        DarwinArm64SymbolInventoryValidationError::DynamicPartitionKindMismatch {
            index: 0,
            expected: DarwinArm64SymbolKindV1::ExternalStrongDefinition,
            actual: DarwinArm64SymbolKindV1::LocalSectionDefinition,
        },
    );
}

fn assert_symbol_error(bytes: &[u8], expected: DarwinArm64SymbolInventoryValidationError) {
    assert_eq!(
        validate_darwin_arm64_object_envelope_v1(bytes),
        Err(ObjectEnvelopeValidationError::SymbolInventory(expected))
    );
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
