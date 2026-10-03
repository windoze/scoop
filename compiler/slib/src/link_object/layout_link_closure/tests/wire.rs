use super::*;
use crate::link_object::layout_link_closure::{
    tests::fixture::{Provider, TARGET},
    verify_external_shape_requirements_v1,
};
use crate::link_object::native_requirements::tests::dependency_closure;
use crate::link_object::strong_relocation_closure::tests::verified_member_with_undefined;
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use scoop_wire::{decode_canonical, encode};

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
    let legacy = dependency_closure(verified_member_with_undefined(&object, symbol.as_bytes()));
    let verified = verify_external_shape_requirements_v1(
        &legacy,
        consumer.selected().consumer(),
        consumer.selected().physical_imports(),
    )
    .unwrap();
    let bytes = encode(&verified.requirements()[0]).unwrap();
    let decode = || decode_canonical::<DecodedUse>(&bytes).unwrap();
    validate_requirements(&[decode()], verified.requirements()).unwrap();
    let mut wrong_index = decode();
    wrong_index.import_index = 1;
    assert!(matches!(
        validate_requirements(&[wrong_index], verified.requirements()),
        Err(LayoutLinkClosureError::RequirementsMismatch)
    ));
    assert!(matches!(
        validate_requirements(&[decode(), decode()], verified.requirements()),
        Err(LayoutLinkClosureError::RequirementsMismatch)
    ));
    assert!(matches!(
        validate_requirements(&[], verified.requirements()),
        Err(LayoutLinkClosureError::RequirementsMismatch)
    ));
    let mut changed = bytes.clone();
    let position = changed
        .windows(symbol.len())
        .position(|part| part == symbol.as_bytes())
        .unwrap();
    changed[position] ^= 1;
    let changed: DecodedUse = decode_canonical(&changed).unwrap();
    assert!(matches!(
        validate_requirements(&[changed], verified.requirements()),
        Err(LayoutLinkClosureError::RequirementsMismatch)
    ));
}

#[test]
fn layout_link_semantic_projection_replay_rejects_import_corruption_and_duplicates() {
    let provider = Provider::new();
    let consumer = provider.consumer(scoop_identity::ConeIdentity::SINGLE_FILE);
    let expected = consumer.selected().physical_imports();
    let bytes = encode(expected).unwrap();
    let raw: DecodedCanonicalExternalShapeLinkImportsV1 = decode_canonical(&bytes).unwrap();
    raw.validate_against(expected).unwrap();
    let mut duplicate = vec![0x82];
    duplicate.extend_from_slice(&bytes[1..]);
    duplicate.extend_from_slice(&bytes[1..]);
    let raw: DecodedCanonicalExternalShapeLinkImportsV1 = decode_canonical(&duplicate).unwrap();
    assert!(raw.validate_against(expected).is_err());
    let mut changed = bytes.clone();
    let provider_id = expected.records()[0].provider();
    let position = changed
        .windows(32)
        .position(|part| part == provider_id.as_array())
        .unwrap();
    changed[position] ^= 1;
    let raw: DecodedCanonicalExternalShapeLinkImportsV1 = decode_canonical(&changed).unwrap();
    assert!(raw.validate_against(expected).is_err());
}
