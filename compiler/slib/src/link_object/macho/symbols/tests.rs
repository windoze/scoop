use super::*;
use crate::{ObjectEnvelopeValidationError, validate_macho_object_envelope_v1};

const DYSYMTAB_OFFSET: usize = 32 + 152 + 24;
const SYMBOL_OFFSET: usize = 32 + 152 + 24 + 80 + 24 + 4 + 8;

#[test]
fn normalizes_strong_definition_and_no_dead_strip() {
    let bytes = super::super::tests::object_with_text_section();
    let envelope =
        validate_macho_object_envelope_v1(scoop_target_data::DARWIN_AARCH64_ID, &bytes).unwrap();
    assert_eq!(envelope.symbols().len(), 1);
    let symbol = &envelope.symbols()[0];
    assert_eq!(symbol.table_index(), 0);
    assert_eq!(symbol.name(), b"_f");
    assert_eq!(symbol.kind(), MachOSymbolKindV1::ExternalStrongDefinition);
    assert_eq!(symbol.section_ordinal(), NonZeroU8::new(1));
    assert_eq!(symbol.value(), 0);
    assert!(!symbol.no_dead_strip());
    assert!(!symbol.private_external());

    let mut retained = bytes;
    write_u16(&mut retained, SYMBOL_OFFSET + 6, macho::N_NO_DEAD_STRIP);
    let retained =
        validate_macho_object_envelope_v1(scoop_target_data::DARWIN_AARCH64_ID, &retained).unwrap();
    assert!(retained.symbols()[0].no_dead_strip());

    let mut alternate = super::super::tests::object_with_text_section();
    write_u16(&mut alternate, SYMBOL_OFFSET + 6, macho::N_ALT_ENTRY);
    let alternate =
        validate_macho_object_envelope_v1(scoop_target_data::DARWIN_AARCH64_ID, &alternate)
            .unwrap();
    assert!(!alternate.symbols()[0].no_dead_strip());

    let mut retained_alternate = super::super::tests::object_with_text_section();
    write_u16(
        &mut retained_alternate,
        SYMBOL_OFFSET + 6,
        macho::N_NO_DEAD_STRIP | macho::N_ALT_ENTRY,
    );
    let retained_alternate = validate_macho_object_envelope_v1(
        scoop_target_data::DARWIN_AARCH64_ID,
        &retained_alternate,
    )
    .unwrap();
    assert!(retained_alternate.symbols()[0].no_dead_strip());

    let mut private_external = super::super::tests::object_with_text_section();
    private_external[SYMBOL_OFFSET + 4] |= macho::N_PEXT;
    let private_external =
        validate_macho_object_envelope_v1(scoop_target_data::DARWIN_AARCH64_ID, &private_external)
            .unwrap();
    assert_eq!(
        private_external.symbols()[0].kind(),
        MachOSymbolKindV1::ExternalStrongDefinition
    );
    assert!(private_external.symbols()[0].private_external());
}

#[test]
fn reads_undefined_symbol_attributes() {
    let mut bytes = super::super::tests::object_with_text_section();
    bytes[SYMBOL_OFFSET + 4] = macho::N_UNDF | macho::N_EXT;
    bytes[SYMBOL_OFFSET + 5] = macho::NO_SECT;
    write_u32(&mut bytes, DYSYMTAB_OFFSET + 20, 0);
    write_u32(&mut bytes, DYSYMTAB_OFFSET + 24, 0);
    write_u32(&mut bytes, DYSYMTAB_OFFSET + 28, 1);

    let envelope =
        validate_macho_object_envelope_v1(scoop_target_data::DARWIN_AARCH64_ID, &bytes).unwrap();
    assert_eq!(
        envelope.symbols()[0].kind(),
        MachOSymbolKindV1::ExternalUndefined
    );
    assert_eq!(envelope.symbols()[0].section_ordinal(), None);

    write_u16(&mut bytes, SYMBOL_OFFSET + 6, macho::N_WEAK_REF);
    let envelope =
        validate_macho_object_envelope_v1(scoop_target_data::DARWIN_AARCH64_ID, &bytes).unwrap();
    assert_eq!(
        envelope.symbols()[0].kind(),
        MachOSymbolKindV1::ExternalUndefined
    );
}

#[test]
fn preserves_external_weak_definition_flags() {
    let mut bytes = super::super::tests::object_with_text_section();
    for flags in [
        macho::N_WEAK_DEF,
        macho::N_WEAK_DEF | macho::N_ALT_ENTRY,
        macho::N_WEAK_DEF | macho::N_NO_DEAD_STRIP | macho::N_ALT_ENTRY,
    ] {
        write_u16(&mut bytes, SYMBOL_OFFSET + 6, flags);
        let envelope =
            validate_macho_object_envelope_v1(scoop_target_data::DARWIN_AARCH64_ID, &bytes)
                .unwrap();
        let symbol = &envelope.symbols()[0];
        assert_eq!(symbol.kind(), MachOSymbolKindV1::ExternalWeakDefinition);
        assert_eq!(symbol.no_dead_strip(), flags & macho::N_NO_DEAD_STRIP != 0);
    }
}

#[test]
fn rejects_invalid_definition_locations() {
    let mut bad_section = super::super::tests::object_with_text_section();
    bad_section[SYMBOL_OFFSET + 5] = 2;
    assert_symbol_error(
        &bad_section,
        MachOSymbolInventoryValidationError::InvalidDefinitionSection {
            index: 0,
            section_ordinal: 2,
        },
    );

    let mut bad_value = super::super::tests::object_with_text_section();
    write_u64(&mut bad_value, SYMBOL_OFFSET + 8, 5);
    assert_symbol_error(
        &bad_value,
        MachOSymbolInventoryValidationError::DefinitionValueOutOfBounds { index: 0 },
    );
}

fn assert_symbol_error(bytes: &[u8], expected: MachOSymbolInventoryValidationError) {
    assert_eq!(
        validate_macho_object_envelope_v1(scoop_target_data::DARWIN_AARCH64_ID, bytes),
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
