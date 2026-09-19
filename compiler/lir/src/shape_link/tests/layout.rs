use super::*;

#[test]
fn shape_link_layout_and_scan_contracts_round_trip_without_post_definition() {
    let record: ExactLayoutExportV1 =
        crate::exact_layout::tests::integer("Word", IntegerKind::SIGNED_64).into();
    let scan = RefScan::None;
    for contract in [
        ShapeLinkContractV1::Layout { record: &record },
        ShapeLinkContractV1::Scan {
            layout: record.identity().layout(),
            role: ScanRole::InlineValue,
            canonical_scan: &scan,
        },
    ] {
        let bytes = encode(&contract).unwrap();
        assert_eq!(bytes[0], if contract.tag() == 2 { 0xa2 } else { 0xa4 });
        let raw: DecodedShapeLinkContractV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&raw).unwrap(), bytes);
        raw.validate_against(&contract, &mut meter()).unwrap();
        let definition = record.identity().physical_definition().definition();
        assert!(!bytes.windows(32).any(|part| part == definition.as_array()));
    }
}

#[test]
fn shape_link_terminal_rebind_rejects_same_id_different_layout_and_scan() {
    let bound = crate::exact_layout::tests::Bound::value(crate::exact_layout::tests::exact(
        &crate::exact_layout::tests::source("Word", SourceNominalKind::Struct, 0),
    ));
    let scalar = |kind| {
        ExactValueLayoutV1::scalar(
            bound.identity.clone(),
            ScalarRepresentationKindV1::Integer(kind),
            &bound.foundation,
            &mut meter(),
        )
        .unwrap()
        .into()
    };
    let first: ExactLayoutExportV1 = scalar(IntegerKind::SIGNED_64);
    let second: ExactLayoutExportV1 = scalar(IntegerKind::SIGNED_32);
    assert_eq!(first.identity().layout(), second.identity().layout());
    let target = LirTargetProfile::DARWIN_AARCH64;
    let layouts = CanonicalExactLayoutExportsV1::try_new(
        target,
        &bound.foundation,
        vec![second],
        &mut meter(),
    )
    .unwrap();
    let callables = CanonicalExactCallableAbiExportsV1::try_new(
        target,
        &bound.foundation,
        Vec::new(),
        &mut meter(),
    )
    .unwrap();
    let descriptors = CanonicalExactDescriptorExportsV1::try_new(
        target,
        &bound.foundation,
        Vec::new(),
        &mut meter(),
    )
    .unwrap();
    let dispatch = CanonicalExactDispatchExportsV1::try_new(
        target,
        &bound.foundation,
        Vec::new(),
        &mut meter(),
    )
    .unwrap();
    let physical = first.identity().physical_definition();
    let mut import = ExternalShapeLinkImportV1 {
        provider: physical.provider(),
        subject: ExternalStrongShapeSubjectV1::Layout(first.identity().layout()),
        expected_symbol: physical.symbol(),
        required_definition: physical.definition(),
        contract: ShapeLinkContractV1::Layout { record: &first },
    };
    assert!(matches!(
        import.validate_semantic_against(
            &layouts,
            &callables,
            &descriptors,
            &dispatch,
            &mut meter()
        ),
        Err(ShapeLinkError::Contract)
    ));
    let invalid_scan = RefScan::References(vec![0]);
    import.subject = ExternalStrongShapeSubjectV1::Scan(first.scan());
    import.contract = ShapeLinkContractV1::Scan {
        layout: first.identity().layout(),
        role: ScanRole::InlineValue,
        canonical_scan: &invalid_scan,
    };
    assert!(matches!(
        import.validate_semantic_against(
            &layouts,
            &callables,
            &descriptors,
            &dispatch,
            &mut meter()
        ),
        Err(ShapeLinkError::Contract)
    ));
    let raw: DecodedShapeLinkContractV1 =
        decode_canonical(&encode(import.contract()).unwrap(), DecodeLimits::default()).unwrap();
    let expected = ShapeLinkContractV1::Scan {
        layout: first.identity().layout(),
        role: ScanRole::InlineValue,
        canonical_scan: &RefScan::None,
    };
    assert!(matches!(
        raw.validate_against(&expected, &mut meter()),
        Err(ShapeLinkError::Contract)
    ));
}
