use super::*;
use crate::link_object::layout_link_closure::{
    tests::fixture::{Provider, TARGET, meter},
    verify_external_shape_requirements_v1,
};
use crate::link_object::native_requirements::tests::dependency_closure;
use crate::link_object::strong_relocation_closure::tests::verified_member_with_undefined;
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use scoop_wire::{DecodeLimits, decode_canonical, encode};

#[test]
fn layout_link_reader_replays_nonempty_uses_without_promoting_wire_fields() {
    let provider = Provider::new();
    let consumer = provider.consumer(scoop_identity::ConeIdentity::SINGLE_FILE);
    let import = &consumer.selected().physical_imports().records()[0];
    let symbol = TARGET
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(import.expected_symbol().symbol().as_str());
    let object = fixture_for_producer(consumer.provider(), "wireUse");
    let legacy = dependency_closure(
        consumer.provider(),
        verified_member_with_undefined(&object, symbol.as_bytes()),
    );
    let verified =
        verify_external_shape_requirements_v1(&legacy, consumer.selected(), &mut meter()).unwrap();
    let bytes = encode(&verified.requirements()[0]).unwrap();
    let decode = || decode_canonical::<DecodedUse>(&bytes, DecodeLimits::default()).unwrap();
    validate_requirements(&[decode()], verified.requirements(), &mut meter()).unwrap();
    let mut wrong_index = decode();
    wrong_index.import_index = 1;
    assert!(matches!(
        validate_requirements(&[wrong_index], verified.requirements(), &mut meter()),
        Err(LayoutLinkClosureError::RequirementsMismatch)
    ));
    assert!(matches!(
        validate_requirements(&[decode(), decode()], verified.requirements(), &mut meter()),
        Err(LayoutLinkClosureError::RequirementsMismatch)
    ));
    assert!(matches!(
        validate_requirements(&[], verified.requirements(), &mut meter()),
        Err(LayoutLinkClosureError::RequirementsMismatch)
    ));
    let mut changed = bytes.clone();
    let position = changed
        .windows(symbol.len())
        .position(|part| part == symbol.as_bytes())
        .unwrap();
    changed[position] ^= 1;
    let changed: DecodedUse = decode_canonical(&changed, DecodeLimits::default()).unwrap();
    assert!(matches!(
        validate_requirements(&[changed], verified.requirements(), &mut meter()),
        Err(LayoutLinkClosureError::RequirementsMismatch)
    ));
}

#[test]
fn layout_link_semantic_projection_replay_rejects_import_corruption_and_duplicates() {
    let provider = Provider::new();
    let consumer = provider.consumer(scoop_identity::ConeIdentity::SINGLE_FILE);
    let expected = consumer.selected().physical_imports();
    let bytes = encode(expected).unwrap();
    let raw: DecodedCanonicalExternalShapeLinkImportsV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    raw.validate_against(expected, &mut meter()).unwrap();
    let mut duplicate = vec![0x82];
    duplicate.extend_from_slice(&bytes[1..]);
    duplicate.extend_from_slice(&bytes[1..]);
    let raw: DecodedCanonicalExternalShapeLinkImportsV1 =
        decode_canonical(&duplicate, DecodeLimits::default()).unwrap();
    assert!(raw.validate_against(expected, &mut meter()).is_err());
    let mut changed = bytes.clone();
    let provider_id = expected.records()[0].provider();
    let position = changed
        .windows(32)
        .position(|part| part == provider_id.as_array())
        .unwrap();
    changed[position] ^= 1;
    let raw: DecodedCanonicalExternalShapeLinkImportsV1 =
        decode_canonical(&changed, DecodeLimits::default()).unwrap();
    assert!(raw.validate_against(expected, &mut meter()).is_err());
}

#[test]
fn layout_link_physical_projection_checks_empty_sets_and_cumulative_budget() {
    let provider = Provider::new();
    let consumer = provider.consumer(scoop_identity::ConeIdentity::SINGLE_FILE);
    let expected = consumer.selected().physical_imports();
    let bytes = encode(expected).unwrap();
    let link = projection(&bytes);
    link.validate_semantic_imports_against(consumer.selected(), &mut meter())
        .unwrap();
    let mut measured = meter();
    link.validate_physical_imports_against(expected, &mut measured)
        .unwrap();
    let mut budget = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    link.validate_physical_imports_against(expected, &mut budget)
        .unwrap();
    assert!(matches!(
        link.validate_physical_imports_against(expected, &mut budget),
        Err(LayoutLinkClosureError::Resource(_))
    ));
    let empty = provider.section.selected().physical_imports();
    projection(&[0x80])
        .validate_physical_imports_against(empty, &mut meter())
        .unwrap();
    assert!(matches!(
        projection(&[0x80]).validate_physical_imports_against(expected, &mut meter()),
        Err(LayoutLinkClosureError::SemanticImports(_))
    ));
    assert!(matches!(
        link.validate_physical_imports_against(empty, &mut meter()),
        Err(LayoutLinkClosureError::SemanticImports(_))
    ));
    let mut duplicate = vec![0x82];
    duplicate.extend_from_slice(&bytes[1..]);
    duplicate.extend_from_slice(&bytes[1..]);
    assert!(matches!(
        projection(&duplicate).validate_physical_imports_against(expected, &mut meter()),
        Err(LayoutLinkClosureError::SemanticImports(_))
    ));
    let mut wrong = bytes.clone();
    let position = wrong
        .windows(32)
        .position(|part| part == expected.records()[0].provider().as_array())
        .unwrap();
    wrong[position] ^= 1;
    assert!(matches!(
        projection(&wrong).validate_physical_imports_against(expected, &mut meter()),
        Err(LayoutLinkClosureError::SemanticImports(_))
    ));
}

fn projection(imports: &[u8]) -> DecodedCrossConeLayoutLinkClosureSectionV1 {
    let mut bytes = vec![0xa3, 0x01];
    bytes.extend_from_slice(imports);
    bytes.extend([0x02, 0x80, 0x03, 0xa2, 0x01, 0x80, 0x02, 0x58, 0x20]);
    bytes.extend([0; 32]);
    decode_canonical(&bytes, DecodeLimits::default()).unwrap()
}
