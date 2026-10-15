use scoop_identity::ConeIdentity;
use scoop_wire::encode;

use super::*;
use crate::link_object::VerifiedCrossConeStrongRequirementClosureV1;
use crate::link_object::native_requirements::tests::dependency_closure;
use crate::link_object::strong_relocation_closure::tests::verified_member_with_undefined;
use crate::link_object::symbol_verification::tests::fixture_for_producer;

pub(in crate::link_object) mod fixture;
use fixture::{Provider, TARGET, empty_section};

mod partitions;

fn single_legacy(
    producer: ConeIdentity,
    symbol: &[u8],
) -> VerifiedCrossConeStrongRequirementClosureV1 {
    let object = fixture_for_producer(producer, "layoutUse");
    dependency_closure(verified_member_with_undefined(&object, symbol))
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
    let verified = verify_external_shape_requirements_v1(
        &legacy,
        consumer.selected().consumer(),
        consumer.selected().physical_imports(),
    )
    .unwrap();
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
fn layout_link_retains_metadata_imports_and_rejects_wrong_consumers() {
    let producer = ConeIdentity::SINGLE_FILE;
    let legacy = single_legacy(producer, b"_native");
    let empty = empty_section(producer);
    let verified = verify_external_shape_requirements_v1(
        &legacy,
        empty.selected().consumer(),
        empty.selected().physical_imports(),
    )
    .unwrap();
    assert!(verified.requirements().is_empty());
    assert_eq!(
        verified.remaining_external_candidates()[0].symbol(),
        b"_native"
    );
    let provider = Provider::new();
    let consumer = provider.consumer(producer);
    let metadata = verify_external_shape_requirements_v1(
        &legacy,
        consumer.selected().consumer(),
        consumer.selected().physical_imports(),
    )
    .unwrap();
    assert!(metadata.requirements().is_empty());
    assert_eq!(metadata.semantic_imports().records().len(), 1);
    assert_eq!(
        metadata.remaining_external_candidates()[0].symbol(),
        b"_native"
    );
    let wrong = empty_section(provider.foundation.producer());
    assert!(matches!(
        verify_external_shape_requirements_v1(
            &legacy,
            wrong.selected().consumer(),
            wrong.selected().physical_imports()
        ),
        Err(LayoutLinkClosureError::ConsumerMismatch { .. })
    ));
}
