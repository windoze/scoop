use super::*;

#[test]
fn parameter_reader_resolves_value_types_and_origins_in_the_same_identity_graph() {
    let (foreign, origin) = super::super::super::source_dispatch::with_hir_source_at(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.scoop"
        )),
        "src/other.scoop",
        |output, _| {
            let export = &output.output().export;
            let class = export.public_surface.classes[0];
            let id = export.nominal_identities[class]
                .source()
                .unwrap()
                .concrete_id()
                .unwrap();
            let protocols = table(output);
            let origin = protocols
                .records()
                .iter()
                .flat_map(|record| record.parameters())
                .next()
                .unwrap()
                .definition_origin()
                .clone();
            (id, origin)
        },
    );
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let record = table
            .records()
            .iter()
            .find(|r| r.parameters().len() == 1)
            .unwrap();
        let parameter = &record.parameters()[0];
        for wrong_origin in [false, true] {
            let shape = if wrong_origin {
                parameter.shape().clone()
            } else {
                hir::SourceParameterShapeV1::new(
                    parameter.shape().name().clone(),
                    scoop_identity::SignatureTypeKey::Nominal(foreign),
                )
            };
            let origin = if wrong_origin {
                origin.clone()
            } else {
                parameter.definition_origin().clone()
            };
            let record = Record::try_new(
                record.owner(),
                vec![hir::InheritanceSourceParameterV1::new(
                    shape,
                    parameter.calling_kind(),
                    origin,
                )],
                &mut meter(),
            )
            .unwrap();
            let bytes = encode(&Records(&[record])).unwrap();
            let decoded: Decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            let result = decoded.resolve(
                &mut source_inventory::identity_closure(output),
                &mut meter(),
            );
            assert!(
                matches!(result, Err(hir::SourceInventoryError::Reference(_))),
                "wrong_origin={wrong_origin}: {result:?}"
            );
        }
    });
}
