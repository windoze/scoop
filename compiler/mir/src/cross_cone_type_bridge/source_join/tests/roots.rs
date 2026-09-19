use super::*;

#[test]
fn ordinary_root_inventory_requires_exact_shape_family_coverage() {
    let mut source = Source::new(ConeIdentity::SINGLE_FILE);
    let exports = source.exports();
    source.roots.clear();
    assert!(matches!(
        exports.validate_sources(
            source.provider,
            &source.fixture.graph,
            MirTypeBridgeShapeRootAuthorityV1::Ordinary,
            &source,
            &mut meter()
        ),
        Err(MirTypeBridgeSourceJoinError::Shape(
            MirShapeSupportError::UnexpectedSource { .. }
        ))
    ));
    source.roots.push(source.fixture.other.id());
    assert!(matches!(
        exports.validate_sources(
            source.provider,
            &source.fixture.graph,
            MirTypeBridgeShapeRootAuthorityV1::Ordinary,
            &source,
            &mut meter()
        ),
        Err(MirTypeBridgeSourceJoinError::SourceRootType { .. })
    ));
}

#[test]
fn core_replays_helpers_from_existing_roots_while_keeping_the_new_table_empty() {
    let source = Source::new(ConeIdentity::CORE);
    let exports = source.exports();
    let bridge = source.core_bridge();
    assert!(exports.shapes.records().is_empty());
    exports
        .validate_sources(
            source.provider,
            &source.fixture.graph,
            MirTypeBridgeShapeRootAuthorityV1::Core(&bridge),
            &source,
            &mut meter(),
        )
        .unwrap();
    assert!(matches!(
        exports.validate_sources(
            source.provider,
            &source.fixture.graph,
            MirTypeBridgeShapeRootAuthorityV1::Ordinary,
            &source,
            &mut meter()
        ),
        Err(MirTypeBridgeSourceJoinError::CoreRootAuthority)
    ));
}

#[test]
fn missing_core_helper_is_rejected_despite_a_complete_legacy_root_record() {
    let mut source = Source::new(ConeIdentity::CORE);
    let mut exports = source.exports();
    let missing = source.fixture.slot_export().exact();
    exports.types = CanonicalParamFreeMirTypeExportsV1::try_new(
        exports
            .types
            .records()
            .iter()
            .filter(|r| r.exact() != missing)
            .cloned()
            .collect(),
    )
    .unwrap();
    source.required_types.retain(|exact| *exact != missing);
    let bridge = source.core_bridge();
    assert!(
        matches!(exports.validate_sources(source.provider, &source.fixture.graph, MirTypeBridgeShapeRootAuthorityV1::Core(&bridge), &source, &mut meter()), Err(MirTypeBridgeSourceJoinError::Shape(MirShapeSupportError::MissingType { exact })) if exact == missing)
    );
}

#[test]
fn core_cannot_substitute_a_second_root_list_for_the_old_authority() {
    let mut source = Source::new(ConeIdentity::CORE);
    let exports = source.exports();
    let bridge = source.core_bridge();
    source.roots.clear();
    assert!(matches!(
        exports.validate_sources(
            source.provider,
            &source.fixture.graph,
            MirTypeBridgeShapeRootAuthorityV1::Core(&bridge),
            &source,
            &mut meter()
        ),
        Err(MirTypeBridgeSourceJoinError::CoreRootAuthority)
    ));
}
