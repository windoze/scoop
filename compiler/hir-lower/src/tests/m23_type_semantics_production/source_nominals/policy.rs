use super::*;

#[test]
fn nominal_source_policy_projects_actual_public_generic_and_nested_declarations() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-nominals/c-layout.scoop"
    ));
    let expected = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-nominals/c-layout.snap"
    ));
    check_source(source, expected);
}

#[test]
fn nominal_source_policy_projects_a_single_c_layout_declaration() {
    check_source(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/c-layout-single.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/c-layout-single.snap"
        )),
    );
}

fn check_source(source: &str, expected: &str) {
    with_source(source, |output, _| {
        let table = table(output);
        let export = output.output().export.module();
        let public = public_interface(output);
        let source_keys = sources(export);
        let mut dump = Vec::new();
        for record in table.records() {
            let hir::NominalSourceShapeV1::Struct(shape) = record.source_shape() else {
                continue;
            };
            let declaration = export
                .structs
                .iter()
                .find(|(id, _)| source_nominal(&export.nominal_identities[*id]) == record.owner())
                .unwrap()
                .1;
            assert_eq!(
                shape.c_layout_policy(),
                hir::NominalCLayoutPolicyV1::from_source_contract(declaration.attributes.c_layout)
            );
            if let Some(public) = public.nominal_interfaces().get(record.owner()) {
                assert_eq!(public.source_shape(), record.source_shape());
            } else {
                assert_eq!(name(source_keys[&record.owner()]), "Header");
            }
            dump.push(format!(
                "{} fields={} policy={:?}\n",
                name(source_keys[&record.owner()]),
                shape.fields().len(),
                shape.c_layout_policy()
            ));
        }
        dump.sort();
        assert_eq!(dump.concat(), expected);
        let bytes = encode(&table).unwrap();
        let decoded: Decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let mut graph = source_inventory::identity_closure(output);
        assert_eq!(decoded.resolve(&mut graph, &mut meter()).unwrap(), table);
    });
}

fn source_nominal(identity: &hir::HirNominalIdentity) -> hir::SourceNominalId {
    hir::SourceNominalId::from_source_declaration(identity.source().unwrap().declaration()).unwrap()
}
