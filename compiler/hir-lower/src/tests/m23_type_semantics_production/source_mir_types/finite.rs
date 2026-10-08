use super::*;

#[test]
fn finite_boxes_export_direct_interfaces_while_machine_dispatch_keeps_the_diamond() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-mir-boxing-production");
    let source = std::fs::read_to_string(directory.join("combined.scoop")).unwrap();
    with_production(&source, |output, input, _, graph, sources| {
        let owners = source_dispatch::owners(output);
        let finite =
            CanonicalParamFreeMirTypeExportsV1::from_generated_shapes(input, sources, graph)
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
                let equality = usize::from(root.shape().exact() == owners["Choice"]);
                assert_eq!(actual.len(), 4 + equality);
                assert_eq!(source.base_and_interfaces().interfaces.len(), 1 + equality);
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
            CanonicalParamFreeMirTypeExportsV1::from_generated_shapes(
                input,
                &missing,
                graph,

            ),
            Err(scoop_mir::MirTypeBridgeError::MissingShapeSupportSource { exact })
                if exact == root.shape().exact()
        ));
    });
}
