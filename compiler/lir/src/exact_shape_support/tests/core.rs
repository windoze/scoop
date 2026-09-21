use super::*;
use scoop_identity::ConeCoordinate;

#[test]
fn core_value_and_reference_shapes_use_common_replay_and_wire() {
    for kind in [SourceNominalKind::Struct, SourceNominalKind::Interface] {
        let fixture = Fixture::for_provider(ConeCoordinate::reserved_core(), kind, false);
        let record = fixture.replay().unwrap();
        assert_eq!(record.provider(), ConeIdentity::CORE);
        assert_eq!(record.source_nominal(), fixture.source_nominal());
        assert_eq!(
            record.roles().boxed_value().available().is_some(),
            kind == SourceNominalKind::Struct
        );
        let table = CanonicalParamFreeShapeSupportExportsV1::from_sources(
            std::slice::from_ref(fixture.source()),
            fixture.layouts(),
            fixture.descriptors(),
            fixture.foundation(),
            &mut fixture.meter(),
        )
        .unwrap();
        assert_eq!(table.get(record.source_nominal()), Some(&record));
        let bytes = encode(&table).unwrap();
        let decoded: DecodedCanonicalParamFreeShapeSupportExportsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let replayed = decoded
            .validate(
                std::slice::from_ref(fixture.source()),
                fixture.layouts(),
                fixture.descriptors(),
                fixture.foundation(),
                &mut fixture.meter(),
            )
            .unwrap();
        assert_eq!(replayed, table);
        assert_eq!(encode(&replayed).unwrap(), bytes);
        let omitted: DecodedCanonicalParamFreeShapeSupportExportsV1 =
            decode_canonical(b"\x80", DecodeLimits::default()).unwrap();
        assert!(matches!(
            omitted.validate(
                std::slice::from_ref(fixture.source()),
                fixture.layouts(),
                fixture.descriptors(),
                fixture.foundation(),
                &mut fixture.meter(),
            ),
            Err(ParamFreeShapeSupportTableError::Coverage)
        ));
    }
}

#[test]
fn core_shapes_require_the_same_descriptor_and_payload_relationships() {
    let fixture = Fixture::for_provider(
        ConeCoordinate::reserved_core(),
        SourceNominalKind::Struct,
        true,
    );
    assert!(matches!(
        fixture.replay(),
        Err(ParamFreeShapeSupportExportError::HelperPayload(_))
    ));
    let fixture = Fixture::for_provider(
        ConeCoordinate::reserved_core(),
        SourceNominalKind::Struct,
        false,
    );
    let missing = fixture
        .replay()
        .unwrap()
        .roles()
        .coroutine_slot()
        .available()
        .unwrap()
        .exact();
    let descriptors = CanonicalExactDescriptorExportsV1::try_new(
        LirTargetProfile::DARWIN_AARCH64,
        fixture.foundation(),
        fixture
            .descriptors()
            .records()
            .iter()
            .filter(|record| record.exact() != missing)
            .cloned()
            .collect(),
        &mut fixture.meter(),
    )
    .unwrap();
    assert!(matches!(ParamFreeShapeSupportExportV1::replay(
        fixture.source(), fixture.layouts(), &descriptors, fixture.foundation(), &mut fixture.meter(),
    ), Err(ParamFreeShapeSupportExportError::MissingDescriptor(actual)) if actual == missing));
    let ordinary = Fixture::new(SourceNominalKind::Struct, false);
    assert!(matches!(
        ParamFreeShapeSupportExportV1::replay(
            ordinary.source(),
            fixture.layouts(),
            fixture.descriptors(),
            fixture.foundation(),
            &mut fixture.meter(),
        ),
        Err(ParamFreeShapeSupportExportError::ForeignSource {
            expected: ConeIdentity::CORE,
            actual: ConeIdentity::SINGLE_FILE
        })
    ));
}
