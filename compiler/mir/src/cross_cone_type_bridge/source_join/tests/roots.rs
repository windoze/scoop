use super::*;

#[test]
fn source_roots_use_common_shape_coverage_for_core_and_ordinary_providers() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let mut source = Source::new(provider);
        let exports = source.exports();
        assert_eq!(exports.shapes.records().len(), 1);
        exports
            .validate_sources(provider, &source.fixture.graph, &source, &mut meter())
            .unwrap();
        source.roots.clear();
        assert!(matches!(
            exports.validate_sources(provider, &source.fixture.graph, &source, &mut meter()),
            Err(MirTypeBridgeSourceJoinError::Shape(
                MirShapeSupportError::UnexpectedSource { source: actual }
            )) if actual == source.fixture.empty.id()
        ));
        source.roots.push(source.fixture.other.id());
        let error = exports
            .validate_sources(provider, &source.fixture.graph, &source, &mut meter())
            .err()
            .expect("an unrelated source root must fail validation");
        match (provider, error) {
            (
                ConeIdentity::CORE,
                MirTypeBridgeSourceJoinError::SourceRootProvider { source: actual },
            )
            | (
                ConeIdentity::SINGLE_FILE,
                MirTypeBridgeSourceJoinError::SourceRootType { source: actual },
            ) => {
                assert_eq!(actual, source.fixture.other.id());
            }
            (_, error) => panic!("unexpected root validation error: {error:?}"),
        }
    }
}

#[test]
fn shape_source_join_rejects_missing_helpers_for_every_provider() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let mut source = Source::new(provider);
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
        assert!(matches!(
            exports.validate_sources(provider, &source.fixture.graph, &source, &mut meter()),
            Err(MirTypeBridgeSourceJoinError::Shape(MirShapeSupportError::MissingType { exact }))
                if exact == missing
        ));
    }
}

#[test]
fn core_shape_table_round_trips_and_rejects_missing_source_records() {
    use scoop_wire::{decode_canonical, encode};

    let mut source = Source::new(ConeIdentity::CORE);
    let mut exports = source.exports();
    let bytes = encode(&exports.shapes).unwrap();
    let decoded: DecodedCanonicalMirShapeSupportsV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let replayed = decoded
        .validate(
            source.provider,
            &mut source.fixture.graph,
            &exports.types,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(replayed, exports.shapes);
    assert_eq!(encode(&replayed).unwrap(), bytes);
    exports.shapes = CanonicalMirShapeSupportsV1::try_new(
        source.provider,
        MirShapeSupportAuthority {
            identities: &source.fixture.graph,
            types: &exports.types,
        },
        vec![],
        &mut meter(),
    )
    .unwrap();
    assert!(matches!(
        exports.validate_sources(source.provider, &source.fixture.graph, &source, &mut meter()),
        Err(MirTypeBridgeSourceJoinError::Shape(MirShapeSupportError::MissingSource { source: actual }))
            if actual == source.fixture.empty.id()
    ));
}
