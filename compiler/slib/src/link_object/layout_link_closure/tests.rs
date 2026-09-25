use scoop_identity::ConeIdentity;
use scoop_lir::StrongExternalLirBridgeSurfaceV1;
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::link_object::native_requirements::tests::dependency_closure;
use crate::link_object::strong_relocation_closure::tests::{
    verified_member_with_undefined, verified_member_without_relocations,
};
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use crate::link_object::{
    VerifiedCrossConeStrongRequirementClosureV1, VerifiedCurrentConeStrongRelocationClosureV1,
    verify_dependency_strong_requirements_v1,
};

pub(in crate::link_object) mod fixture;
use fixture::{Provider, TARGET, empty_section};

mod partitions;
mod terminal;

fn legacy_for(
    strong: VerifiedCurrentConeStrongRelocationClosureV1,
) -> VerifiedCrossConeStrongRequirementClosureV1 {
    let producer = strong.producer();
    let object = fixture_for_producer(producer, "coreOwnerSeed");
    let seed = dependency_closure(producer, verified_member_without_relocations(&object));
    verify_dependency_strong_requirements_v1(
        TARGET,
        strong,
        StrongExternalLirBridgeSurfaceV1::try_new(producer, vec![]).unwrap(),
        seed.dependency_owners(),
    )
    .unwrap()
}

fn single_legacy(
    producer: ConeIdentity,
    symbol: &[u8],
) -> VerifiedCrossConeStrongRequirementClosureV1 {
    let object = fixture_for_producer(producer, "layoutUse");
    dependency_closure(producer, verified_member_with_undefined(&object, symbol))
}

#[test]
fn layout_link_classifies_a_real_layout_import_and_preserves_its_compile_table() {
    let provider = Provider::new();
    let consumer = provider.consumer(ConeIdentity::SINGLE_FILE);
    let import = &consumer.selected().physical_imports().records()[0];
    let symbol = TARGET
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(import.expected_symbol().symbol().as_str());
    let legacy = single_legacy(consumer.provider(), symbol.as_bytes());
    let verified = verify_external_shape_requirements_v1(&legacy, consumer.selected()).unwrap();
    assert_eq!(verified.requirements().len(), 1);
    assert_eq!(verified.requirements()[0].import_index(), 0);
    assert_eq!(
        verified.requirements()[0].use_site().symbol(),
        symbol.as_bytes()
    );
    assert!(verified.remaining_external_candidates().is_empty());
    assert!(std::ptr::eq(
        verified.semantic_imports(),
        consumer.selected().physical_imports()
    ));
    assert_eq!(
        encode(verified.semantic_imports()).unwrap(),
        encode(consumer.selected().physical_imports()).unwrap()
    );
}

#[test]
fn layout_link_keeps_native_candidates_and_rejects_unused_or_wrong_consumer_imports() {
    let producer = ConeIdentity::SINGLE_FILE;
    let legacy = single_legacy(producer, b"_native");
    let empty = empty_section(producer);
    let verified = verify_external_shape_requirements_v1(&legacy, empty.selected()).unwrap();
    assert!(verified.requirements().is_empty());
    assert_eq!(
        verified.remaining_external_candidates()[0].symbol(),
        b"_native"
    );
    let provider = Provider::new();
    let consumer = provider.consumer(producer);
    assert!(matches!(
        verify_external_shape_requirements_v1(&legacy, consumer.selected()),
        Err(LayoutLinkClosureError::UnusedImport {
            import_index: 0,
            ..
        })
    ));
    let wrong = empty_section(provider.foundation.producer());
    assert!(matches!(
        verify_external_shape_requirements_v1(&legacy, wrong.selected()),
        Err(LayoutLinkClosureError::ConsumerMismatch { .. })
    ));
}

#[test]
fn layout_link_reader_binds_nonempty_final_objects_and_rejects_corruption() {
    let objects =
        crate::link_decode::tests::layout_link_support::verified_code_link_object_members();
    assert!(!objects.members().is_empty());
    let legacy = legacy_for(
        objects
            .final_objects()
            .entry()
            .patch_sites()
            .builtins()
            .strong_relocations()
            .clone(),
    );
    let consumer = empty_section(objects.producer());
    let verified = verify_external_shape_requirements_v1(&legacy, consumer.selected()).unwrap();
    let section =
        CrossConeLayoutLinkClosureSectionV1::from_verified_requirements(&verified, &objects)
            .unwrap();
    let bytes = encode(&section).unwrap();
    let raw: DecodedCrossConeLayoutLinkClosureSectionV1 = decode_canonical(&bytes).unwrap();
    raw.validate_semantic_imports_against(consumer.selected())
        .unwrap();
    let replayed = raw.validate_against(&section).unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);
    let mut changed = bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    let raw: DecodedCrossConeLayoutLinkClosureSectionV1 = decode_canonical(&changed).unwrap();
    assert!(matches!(
        raw.validate_against(&section),
        Err(LayoutLinkClosureError::ObjectCoverageMismatch)
    ));
    let mut changed = bytes.clone();
    let fingerprint = objects.members()[0].fingerprint();
    let offset = changed
        .windows(32)
        .position(|part| part == fingerprint.as_array())
        .unwrap();
    changed[offset] ^= 1;
    let raw: DecodedCrossConeLayoutLinkClosureSectionV1 = decode_canonical(&changed).unwrap();
    assert!(matches!(
        raw.validate_against(&section),
        Err(LayoutLinkClosureError::ObjectCoverageMismatch)
    ));
}

#[test]
fn layout_link_final_objects_must_come_from_the_classified_relocation_proof() {
    let objects =
        crate::link_decode::tests::layout_link_support::verified_code_link_object_members();
    let consumer = empty_section(objects.producer());
    let wrong = single_legacy(objects.producer(), b"_native");
    let verified = verify_external_shape_requirements_v1(&wrong, consumer.selected()).unwrap();
    assert!(matches!(
        CrossConeLayoutLinkClosureSectionV1::from_verified_requirements(&verified, &objects),
        Err(LayoutLinkClosureError::ObjectProofMismatch)
    ));
}

#[test]
fn layout_link_strict_schema_rejects_malformed_input() {
    for bytes in [
        vec![0xa2],
        vec![0xa4],
        vec![0xa3, 1, 0x80, 1, 0x80, 3, 0xa2],
        vec![0xa3, 1, 0x80, 2, 0x81, 0xa1, 1, 0xa0],
    ] {
        assert!(decode_canonical::<DecodedCrossConeLayoutLinkClosureSectionV1>(&bytes).is_err());
    }
}
