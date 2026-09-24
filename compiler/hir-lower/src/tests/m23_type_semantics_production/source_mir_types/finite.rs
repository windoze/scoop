use super::*;

#[test]
fn finite_boxes_export_direct_interfaces_while_machine_dispatch_keeps_the_diamond() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-mir-boxing-production");
    let source = std::fs::read_to_string(directory.join("combined.scoop")).unwrap();
    with_production(&source, |_, input, _, graph, sources| {
        let finite = CanonicalParamFreeMirTypeExportsV1::from_finite_shape_support(
            input,
            sources,
            graph,
            &mut BudgetMeter::new(DecodeLimits::default()),
        )
        .unwrap();
        let mut diamonds = 0;
        for root in input.materialization().shape_support() {
            let scoop_mir::StrongBoxedShapeSupportRoot::Available(boxed) = root.boxed() else {
                continue;
            };
            let scoop_mir::GeneratedExactTypeLocation::Class(class) = boxed.location() else {
                panic!("a finite box is a class");
            };
            let source = sources.get(root.shape().exact()).unwrap();
            let helper = finite.get(boxed.exact()).unwrap();
            assert_eq!(
                helper.base_and_interfaces().interfaces,
                source.base_and_interfaces().interfaces
            );
            let actual = &input.module().classes[class].interfaces;
            if actual.len() > source.base_and_interfaces().interfaces.len() {
                assert_eq!(actual.len(), 4);
                assert_eq!(source.base_and_interfaces().interfaces.len(), 1);
                diamonds += 1;
            }
        }
        assert_eq!(diamonds, 2);

        let root = &input.materialization().shape_support()[0];
        let missing = CanonicalParamFreeMirTypeExportsV1::try_new(
            sources
                .records()
                .iter()
                .filter(|record| record.exact() != root.shape().exact())
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            CanonicalParamFreeMirTypeExportsV1::from_finite_shape_support(
                input,
                &missing,
                graph,
                &mut BudgetMeter::new(DecodeLimits::default()),
            ),
            Err(scoop_mir::MirTypeBridgeError::MissingShapeSupportSource { exact })
                if exact == root.shape().exact()
        ));
    });
}
